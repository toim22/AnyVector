//! The effects that change how the image looks or its shape: 3D Effects, Art Strokes, Camera,
//! Color Transform ▸ Halftone, Creative, Custom ▸ Bump Map, Distort and Texture.
//!
//! The textures, frames and displacement maps are drawn here (procedural), not read from files.

use crate::bitfx::*;
use std::f32::consts::{PI, TAU};
use vector_core::object::BitmapEffect;
use vector_core::Color;

/// The names of the textures of Canvas, Bump Map and Displace, in the order of their lists.
pub const TEXTURES: &[&str] = &["Canvas", "Burlap", "Paper", "Bricks", "Waves", "Dots", "Blotches", "Stucco"];
/// The names of the frames of Creative ▸ Frame.
pub const FRAMES: &[&str] = &["Soft Edge", "Rough Edge", "Torn Paper", "Brush Border", "Rounded", "Film"];

fn pct(e: &BitmapEffect, k: &str, d: f64) -> f32 {
    (e.num(k, d) / 100.0) as f32
}
fn n(e: &BitmapEffect, k: &str, d: f64) -> f32 {
    e.num(k, d) as f32
}
fn ix(e: &BitmapEffect, k: &str, d: usize) -> usize {
    e.num(k, d as f64).round().max(0.0) as usize
}

/// Effect-pixel coordinates of the center of a layer pixel.
fn ep(cx: &FxCtx, x: usize, y: usize) -> (f32, f32) {
    let s = cx.s.max(1e-3);
    ((x as f32 + 0.5 - cx.frame[0]) / s, (y as f32 + 0.5 - cx.frame[1]) / s)
}
/// Layer pixel position of an effect-pixel position.
fn lp(cx: &FxCtx, ex: f32, ey: f32) -> (f32, f32) {
    (cx.frame[0] + ex * cx.s, cx.frame[1] + ey * cx.s)
}

/// A new image from a function of each pixel; the alpha of `src` is kept.
fn build(src: &Img, mut f: impl FnMut(usize, usize, [f32; 4]) -> [f32; 3]) -> Img {
    let mut o = src.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            let i = y * src.w + x;
            let p = src.p[i];
            if p[3] <= 0.0 {
                continue;
            }
            let c = f(x, y, p);
            o.p[i] = [clamp255(c[0]), clamp255(c[1]), clamp255(c[2]), p[3]];
        }
    }
    o
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn rgb(p: [f32; 4]) -> [f32; 3] {
    [p[0], p[1], p[2]]
}
fn gray3(v: f32) -> [f32; 3] {
    [v, v, v]
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn edge_mag(im: &Img) -> Vec<f32> {
    let (gx, gy) = sobel(im);
    gx.iter().zip(gy.iter()).map(|(a, b)| ((a * a + b * b).sqrt() / 255.0).min(4.0)).collect()
}
/// Luminance at a layer position (nearest pixel, clamped).
fn lget(l: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let xi = (x.floor() as i64).clamp(0, w as i64 - 1) as usize;
    let yi = (y.floor() as i64).clamp(0, h as i64 - 1) as usize;
    l[yi * w + xi]
}

pub struct Cell {
    pub d1: f32,
    pub d2: f32,
    pub id: (i64, i64),
    pub c: (f32, f32),
}

/// Jittered-grid Voronoi cells of `size` (effect px): the nearest and second nearest distances,
/// the nearest cell and its center.
pub fn voronoi(x: f32, y: f32, size: f32, seed: u32) -> Cell {
    let size = size.max(0.5);
    let (gx, gy) = ((x / size).floor() as i64, (y / size).floor() as i64);
    let mut best = Cell { d1: f32::MAX, d2: f32::MAX, id: (0, 0), c: (0.0, 0.0) };
    for j in -1..=1 {
        for i in -1..=1 {
            let (ci, cj) = (gx + i, gy + j);
            let px = (ci as f32 + 0.1 + 0.8 * hash(ci, cj, seed)) * size;
            let py = (cj as f32 + 0.1 + 0.8 * hash(ci, cj, seed ^ 0x5bd1)) * size;
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < best.d1 {
                best.d2 = best.d1;
                best.d1 = d;
                best.id = (ci, cj);
                best.c = (px, py);
            } else if d < best.d2 {
                best.d2 = d;
            }
        }
    }
    best
}

/// A tileable-looking height texture 0..1 at effect-pixel position (u, v).
pub fn texture(kind: usize, u: f32, v: f32, seed: u32) -> f32 {
    match kind {
        // canvas weave
        0 => {
            let a = ((u * 1.6).sin() * 0.5 + 0.5) * ((v * 0.8).sin() * 0.5 + 0.5);
            let b = ((v * 1.6).sin() * 0.5 + 0.5) * ((u * 0.8 + 1.5).sin() * 0.5 + 0.5);
            (a.max(b) * 0.8 + noise(u * 0.7, v * 0.7, seed) * 0.2).clamp(0.0, 1.0)
        }
        // burlap: a coarse uneven weave
        1 => {
            let a = (u * 0.7 + noise(u * 0.1, v * 0.1, seed) * 3.0).sin() * 0.5 + 0.5;
            let b = (v * 0.7 + noise(v * 0.1, u * 0.1, seed + 1) * 3.0).sin() * 0.5 + 0.5;
            (a * b * 0.7 + fbm(u * 0.3, v * 0.3, 3, seed) * 0.3).clamp(0.0, 1.0)
        }
        // paper fibers
        2 => (fbm(u * 0.15, v * 0.15, 4, seed) * 0.7 + noise(u * 1.3, v * 0.2, seed + 7) * 0.3).clamp(0.0, 1.0),
        // bricks
        3 => {
            let (bw, bh) = (24.0, 10.0);
            let row = (v / bh).floor();
            let uu = u + if row as i64 % 2 == 0 { 0.0 } else { bw / 2.0 };
            let fx = (uu / bw).fract() * bw;
            let fy = (v / bh).fract() * bh;
            let m = fx.min(bw - fx).min(fy.min(bh - fy));
            smooth(0.0, 2.0, m) * (0.85 + 0.15 * noise(u * 0.4, v * 0.4, seed))
        }
        // waves
        4 => ((u * 0.25 + (v * 0.12).sin() * 3.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0),
        // dots
        5 => {
            let (fx, fy) = ((u / 8.0).fract() - 0.5, (v / 8.0).fract() - 0.5);
            1.0 - smooth(0.2, 0.4, (fx * fx + fy * fy).sqrt())
        }
        // blotches
        6 => smooth(0.4, 0.6, fbm(u * 0.04, v * 0.04, 4, seed)),
        // stucco
        _ => (fbm(u * 0.2, v * 0.2, 5, seed).powf(1.5) * 1.4).clamp(0.0, 1.0),
    }
}

/// Applies one of these effects; false when the id is not one of them.
pub fn apply(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) -> bool {
    match e.id.as_str() {
        "ThreeDRotate" | "Cylinder" | "Emboss" | "PageCurl" | "PinchPunch" | "Sphere" | "Zigzag" => three_d(e, im, cx),
        "Charcoal" | "ConteCrayon" | "Crayon" | "Cubist" | "Dabble" | "Impressionist" | "PaletteKnife" | "Pastels" | "PenInk" | "Pointillist" | "ScraperBoard" | "SketchPad" | "Watercolor"
        | "WaterMarker" | "WavePaper" => strokes(e, im, cx),
        "Colorize" | "Diffuse" | "PhotoFilter" | "LensFlare" | "Lighting" | "Sepia" | "SpotFilter" | "TimeMachine" | "Halftone" => camera(e, im, cx),
        "NNArtStyle" | "Crystallize" | "Fabric" | "Frame" | "GlassBlock" | "Mosaic" | "Scatter" | "SmokedGlass" | "StainedGlass" | "Vignette" | "Vortex" => creative(e, im, cx),
        "Blocks" | "Displace" | "MeshWarp" | "Offset" | "Pixelate" | "Ripple" | "Shear" | "Swirl" | "Tile" | "WetPaint" | "Whirlpool" | "Wind" => distort(e, im, cx),
        "BrickWall" | "Bubbles" | "Canvas" | "CobbleStone" | "ElephantSkin" | "Etching" | "Plastic" | "PlasterWall" | "ReliefSculpture" | "ScreenDoor" | "Stone" | "Underpainting" | "BumpMap" => {
            texture_fx(e, im, cx)
        }
        _ => return false,
    }
    true
}

// ------------------------------------------------------------------------------ 3D Effects

fn three_d(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let (ccx, ccy) = cx.at(0.5, 0.5);
    let (hw, hh) = (cx.fw() / 2.0, cx.fh() / 2.0);
    match e.id.as_str() {
        "ThreeDRotate" => {
            let (v, h) = (n(e, "VertAngle", 0.0).to_radians(), n(e, "HorizAngle", 0.0).to_radians());
            if v == 0.0 && h == 0.0 {
                return;
            }
            let f = cx.fw().max(cx.fh()) * 1.6;
            let (sh, ch) = h.sin_cos();
            let (sv, cv) = v.sin_cos();
            // the plane turned around the vertical axis by h, then around the horizontal axis by v
            let proj = |x: f32, y: f32| {
                let (px, py, pz) = (x * ch, y * cv + x * sh * sv, y * sv - x * sh * cv);
                let k = f / (f + pz).max(1.0);
                (px * k, py * k)
            };
            let mut scale = 1.0;
            if e.flag("BestFit", true) {
                let (mut mx, mut my) = (1.0f32, 1.0f32);
                for (a, b) in [(-hw, -hh), (hw, -hh), (-hw, hh), (hw, hh)] {
                    let (px, py) = proj(a, b);
                    mx = mx.max(px.abs());
                    my = my.max(py.abs());
                }
                scale = (hw / mx).min(hh / my);
            }
            let nrm = (sh, -ch * sv, ch * cv);
            *im = im.remap(Edge::Clear, |px, py| {
                let d = ((px - ccx) / scale, (py - ccy) / scale, f);
                let nd = nrm.0 * d.0 + nrm.1 * d.1 + nrm.2 * d.2;
                if nd.abs() < 1e-6 {
                    return (-1e9, -1e9);
                }
                let t = nrm.2 * f / nd;
                if t <= 0.0 {
                    return (-1e9, -1e9);
                }
                let q = (d.0 * t, d.1 * t, -f + d.2 * t);
                (ccx + q.0 * ch + q.1 * sh * sv - q.2 * sh * cv, ccy + q.1 * cv + q.2 * sv)
            });
        }
        "Cylinder" => {
            let p = pct(e, "Percentage", 25.0);
            let vert = e.num("Mode", 0.0) >= 1.0;
            let m = |t: f32| {
                if t.abs() > 1.0 {
                    return t;
                }
                if p >= 0.0 {
                    t + p * (t.asin() * 2.0 / PI - t)
                } else {
                    t - p * ((t * PI / 2.0).sin() - t)
                }
            };
            *im = im.remap(Edge::Clear, |x, y| if vert { (x, ccy + m((y - ccy) / hh) * hh) } else { (ccx + m((x - ccx) / hw) * hw, y) });
        }
        "Emboss" => {
            let depth = n(e, "Depth", 2.0).max(1.0) * cx.s;
            let lv = pct(e, "Level", 100.0);
            let dir = n(e, "Direction", 45.0).to_radians();
            let (dx, dy) = (dir.cos() * depth, -dir.sin() * depth);
            let l = im.luma();
            let mode = ix(e, "ColorMode", 1);
            let other = col(e.color("ColorItem", Color::WHITE));
            let (w, h) = (im.w, im.h);
            *im = build(im, |x, y, p| {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let r = (lget(&l, w, h, fx + dx, fy + dy) - lget(&l, w, h, fx - dx, fy - dy)) * lv;
                let base = match mode {
                    0 => rgb(p),
                    1 => gray3(128.0),
                    2 => gray3(30.0),
                    _ => other,
                };
                [base[0] + r, base[1] + r, base[2] + r]
            });
        }
        "PageCurl" => {
            let corner = ix(e, "CornerType", 1);
            let horiz = e.num("DirectionType", 0.0) >= 1.0;
            let transparent = e.num("PaperType", 1.0) >= 1.0;
            let curl = col(e.color("PaintColorItem", Color::rgb(128, 128, 128)));
            let back = col(e.color("BackColorItem", Color::WHITE));
            let (w, h) = ((cx.fw() * pct(e, "Width", 50.0)).max(1.0), (cx.fh() * pct(e, "Height", 50.0)).max(1.0));
            let [fx0, fy0, fx1, fy1] = cx.frame;
            let (kx, ky, sx, sy) = match corner {
                0 => (fx0, fy0, 1.0, 1.0),
                1 => (fx1, fy0, -1.0, 1.0),
                2 => (fx0, fy1, 1.0, -1.0),
                _ => (fx1, fy1, -1.0, -1.0),
            };
            let nlen = ((1.0 / w).powi(2) + (1.0 / h).powi(2)).sqrt();
            let (nx, ny) = ((1.0 / w) / nlen, (1.0 / h) / nlen);
            let maxd = 1.0 / nlen;
            // a vertical curl rolls tighter than a horizontal one
            let squash = if horiz { 0.45 } else { 0.62 };
            let src = im.clone();
            for y in 0..im.h {
                for x in 0..im.w {
                    let i = y * im.w + x;
                    let (a, b) = ((x as f32 + 0.5 - kx) * sx, (y as f32 + 0.5 - ky) * sy);
                    let d = (a / w + b / h - 1.0) / nlen;
                    if d < 0.0 {
                        if a >= 0.0 && b >= 0.0 {
                            im.p[i] = if transparent { [0.0; 4] } else { [back[0], back[1], back[2], src.p[i][3]] };
                        }
                        continue;
                    }
                    let dm = d / squash;
                    let (ma, mb) = (a - (d + dm) * nx, b - (d + dm) * ny);
                    if ma >= 0.0 && mb >= 0.0 && dm <= maxd {
                        let t = dm / maxd;
                        let k = 0.62 + 0.5 * (t * PI).sin() - 0.25 * t;
                        im.p[i] = [clamp255(curl[0] * k), clamp255(curl[1] * k), clamp255(curl[2] * k), 255.0];
                    } else if d < maxd * squash * 1.15 && src.p[i][3] > 0.0 {
                        // the shadow of the curl on the page
                        let k = 1.0 - 0.35 * (1.0 - (d / (maxd * squash * 1.15))).powi(2);
                        for c in 0..3 {
                            im.p[i][c] = src.p[i][c] * k;
                        }
                    }
                }
            }
        }
        "PinchPunch" | "Sphere" => {
            let (ox, oy) = cx.center(e);
            let r0 = hw.min(hh);
            let sphere = e.id == "Sphere";
            let k = if sphere { pct(e, "Percentage", 25.0) } else { pct(e, "PinchPunch", 100.0) };
            *im = im.remap(Edge::Clear, |x, y| {
                let (dx, dy) = (x - ox, y - oy);
                let r = (dx * dx + dy * dy).sqrt() / r0;
                if r >= 1.0 || r < 1e-6 {
                    return (x, y);
                }
                let sr = if sphere && k >= 0.0 {
                    r * (1.0 - k + k * (1.0 - (1.0 - r * r).sqrt()) / r)
                } else if sphere {
                    r.powf(2f32.powf(k))
                } else {
                    r.powf(2f32.powf(-k))
                };
                (ox + dx / r * sr, oy + dy / r * sr)
            });
        }
        "Zigzag" => {
            let (ox, oy) = cx.center(e);
            let typ = ix(e, "WaveType", 1);
            let waves = n(e, "Waves", 20.0).max(1.0);
            let r0 = (hw * hw + hh * hh).sqrt();
            let amp = pct(e, "Strength", 20.0) * r0 * 0.04;
            let damp = pct(e, "Damping", 0.0);
            *im = im.remap(Edge::Clear, |x, y| {
                let (dx, dy) = (x - ox, y - oy);
                let r = (dx * dx + dy * dy).sqrt();
                if r < 1e-3 {
                    return (x, y);
                }
                let t = r / r0;
                let mut off = amp * (t * waves * PI).sin() * (1.0 - damp * t).max(0.0);
                match typ {
                    // pond ripples fade out from the center
                    0 => {
                        off *= (1.0 - t).max(0.0);
                        (x - dx / r * off, y - dy / r * off)
                    }
                    1 => (x - dx / r * off, y - dy / r * off),
                    _ => {
                        let a = off / r;
                        let (s, c) = a.sin_cos();
                        (ox + dx * c - dy * s, oy + dx * s + dy * c)
                    }
                }
            });
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------ Art Strokes

/// Strokes along `angle` (degrees) of `len` layer pixels.
fn strokes_along(im: &Img, angle: f32, len: f32) -> Img {
    let n = (len.ceil() as usize).clamp(1, 48);
    if n <= 1 {
        return im.clone();
    }
    let a = angle.to_radians();
    let (dx, dy) = (a.cos() * len / n as f32, -a.sin() * len / n as f32);
    streak(im, n, |k| (dx * (k as f32 - n as f32 / 2.0), dy * (k as f32 - n as f32 / 2.0)), Edge::Clamp)
}

fn strokes(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let s = cx.s.max(1e-3);
    let seed = cx.seed;
    match e.id.as_str() {
        "Charcoal" => {
            let size = n(e, "Size", 5.0);
            let edge = n(e, "Edge", 0.0);
            let st = strokes_along(im, 45.0, size * s * 2.0);
            let em = edge_mag(im);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let l = luma(&st.at(x, y));
                let mut v = (l - 128.0) * 1.35 + 150.0;
                v -= em[y * st.w + x] * edge * 30.0;
                v += (noise(ex * 0.9, ey * 0.9, seed) - 0.5) * 50.0;
                gray3(v)
            });
        }
        "ConteCrayon" => {
            let crayons = [("Black", [35.0, 32.0, 30.0]), ("Sanguine", [160.0, 60.0, 40.0]), ("Bistre", [70.0, 50.0, 35.0]), ("Sepia", [115.0, 70.0, 25.0])];
            let dark = crayons.iter().find(|c| e.flag(c.0, c.0 == "Black")).map(|c| c.1).unwrap_or([35.0, 32.0, 30.0]);
            let white = e.flag("White", false);
            let paper = col(e.color("PaperColorItem", Color::rgb(236, 226, 200)));
            let pressure = pct(e, "Pressure", 50.0);
            let tex = pct(e, "Texture", 0.0);
            let st = strokes_along(im, 80.0, 6.0 * s);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let v = luma(&st.at(x, y)) / 255.0;
                let grain = noise(ex * 1.4, ey * 0.25, seed) * (0.5 + tex) + noise(ex * 0.3, ey * 0.3, seed + 3) * 0.3;
                let cover = ((1.0 - v) * (0.6 + pressure) - grain * 0.35).clamp(0.0, 1.0);
                let mut c = lerp3(paper, dark, cover);
                if white {
                    c = lerp3(c, [252.0; 3], ((v - 0.65) * 3.0 - grain * 0.3).clamp(0.0, 1.0));
                }
                c
            });
        }
        "Crayon" => {
            let size = n(e, "Size", 12.0);
            let outline = pct(e, "Outline", 25.0) * 2.0;
            let b = gauss(im, size * s * 0.25);
            let em = edge_mag(&b);
            *im = build(&b, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let q = |v: f32| ((v / 255.0 * 6.0).round() / 6.0) * 255.0;
                let grain = noise(ex * 0.8, ey * 0.8, seed);
                let mut c = [q(p[0]), q(p[1]), q(p[2])];
                if grain > 0.78 {
                    c = lerp3(c, [250.0; 3], (grain - 0.78) * 4.0);
                }
                let d = em[y * b.w + x] * outline * 90.0;
                [c[0] - d, c[1] - d, c[2] - d]
            });
        }
        "Cubist" => {
            let size = n(e, "Size", 10.0).max(1.0) * 2.5;
            let bright = 0.5 + pct(e, "Brightness", 50.0);
            let paper = col(e.color("PaperColorItem", Color::BLACK));
            let b = gauss(im, size * s * 0.3);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                // tilted squares: Voronoi in a rotated frame gives straight-edged pieces
                let a = hash((ex / size) as i64, (ey / size) as i64, seed) * 0.6;
                let (u, v) = (ex * a.cos() - ey * a.sin(), ex * a.sin() + ey * a.cos());
                let c = voronoi(u, v, size, seed);
                if c.d2 - c.d1 < 1.2 {
                    return paper;
                }
                let (cu, cv) = c.c;
                let (sx0, sy0) = (cu * a.cos() + cv * a.sin(), -cu * a.sin() + cv * a.cos());
                let (lx, ly) = lp(cx, sx0, sy0);
                let p = b.sample(lx, ly, Edge::Clamp);
                [p[0] * bright, p[1] * bright, p[2] * bright]
            });
        }
        "Dabble" => {
            let size = n(e, "Size", 1.0).max(1.0);
            let sd = seed ^ (e.num("Randomize", 1.0) as u32).wrapping_mul(7919);
            let src = gauss(im, s * 0.6);
            *im = src.remap(Edge::Clamp, |x, y| {
                let (ex, ey) = ((x - cx.frame[0]) / s, (y - cx.frame[1]) / s);
                let f = 3.0 + size;
                let dx = (noise(ex / f, ey / f, sd) - 0.5) * size * 5.0;
                let dy = (noise(ex / f, ey / f, sd + 11) - 0.5) * size * 5.0;
                (x + dx * s, y + dy * s)
            });
        }
        "Impressionist" => {
            let dabs = e.num("Style", 0.0) >= 1.0;
            let size = 3.0 + n(e, "Detail", 33.0) / 6.0;
            let colr = pct(e, "Coloration", 5.0);
            let bright = 0.5 + pct(e, "Brightness", 50.0);
            let src = im.clone();
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                let jitter = hash(c.id.0, c.id.1, seed + 5);
                let p = if dabs {
                    let (lx, ly) = lp(cx, c.c.0, c.c.1);
                    src.sample(lx, ly, Edge::Clamp)
                } else {
                    // a short stroke in the cell's own direction
                    let a = jitter * PI;
                    let (dx, dy) = (a.cos() * size * s * 0.25, a.sin() * size * s * 0.25);
                    let mut acc = [0.0f32; 4];
                    for k in -3..=3 {
                        let q = src.sample(x as f32 + 0.5 + dx * k as f32, y as f32 + 0.5 + dy * k as f32, Edge::Clamp);
                        for c in 0..4 {
                            acc[c] += q[c] / 7.0;
                        }
                    }
                    acc
                };
                let (h, sat, l) = rgb_to_hsl(rgb(p));
                let o = hsl_to_rgb(h + (jitter - 0.5) * colr * 120.0, (sat * (1.0 + colr * 2.0)).min(1.0), l);
                [o[0] * bright, o[1] * bright, o[2] * bright]
            });
        }
        "PaletteKnife" => {
            let size = n(e, "Size", 30.0).max(1.0) * 0.6;
            let soft = n(e, "Softness", 0.0);
            let a = n(e, "Angle", 180.0).to_radians();
            let b = gauss(im, s * 1.5);
            let (ca, sa) = (a.cos(), a.sin());
            let mut out = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                // cells stretched along the knife's direction
                let (u, v) = (ex * ca - ey * sa, ex * sa + ey * ca);
                let row = (v / (size / 3.0)).floor();
                let off = hash(0, row as i64, seed) * size;
                let colm = ((u + off) / size).floor();
                let (cu, cv) = ((colm + 0.5) * size - off, (row + 0.5) * size / 3.0);
                let (sx0, sy0) = (cu * ca + cv * sa, -cu * sa + cv * ca);
                let (lx, ly) = lp(cx, sx0, sy0);
                rgb(b.sample(lx, ly, Edge::Clamp))
            });
            if soft > 0.0 {
                out = gauss(&out, soft * s * 0.6);
                keep_alpha(&mut out, im);
            }
            *im = out;
        }
        "Pastels" => {
            let oil = e.num("PastelType", 0.0) >= 1.0;
            let size = n(e, "Size", 5.0).max(1.0);
            let hv = pct(e, "HueVariation", 30.0);
            let st = strokes_along(im, 60.0, size * s * 1.6);
            *im = build(&st, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let (h, sat, l) = rgb_to_hsl(rgb(p));
                let j = noise(ex / (size * 0.8), ey / (size * 2.0), seed) - 0.5;
                let o = hsl_to_rgb(h + j * hv * 80.0, if oil { (sat * 1.15).min(1.0) } else { sat * 0.85 }, l);
                let grain = noise(ex * 1.1, ey * 1.1, seed + 9);
                lerp3(o, [250.0, 248.0, 240.0], if oil { 0.0 } else { 0.18 } + (grain - 0.7).max(0.0) * 1.5)
            });
        }
        "PenInk" => {
            let stipple = e.num("Style", 0.0) >= 1.0;
            let dens = pct(e, "Density", 75.0);
            let pools = pct(e, "InkPools", 75.0);
            *im = build(im, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let mut t = 1.0 - luma(&p) / 255.0;
                t = (t + t.powi(3) * pools).min(1.0);
                let ink = if stipple {
                    hash(ex.floor() as i64, ey.floor() as i64, seed) < t * (0.4 + dens)
                } else {
                    let sp = 4.5 - dens * 2.0;
                    let line = |v: f32| ((v / sp).fract() - 0.5).abs() * 2.0;
                    (t > 0.15 && line(ex + ey) < t * 0.9) || (t > 0.45 && line(ex - ey) < (t - 0.3) * 0.9) || (t > 0.75 && line(ey) < (t - 0.6) * 1.2)
                };
                gray3(if ink { 15.0 } else { 252.0 })
            });
        }
        "Pointillist" => {
            let size = n(e, "Size", 5.0).max(1.0) * 1.5;
            let bright = 0.5 + pct(e, "Brightness", 50.0);
            let b = gauss(im, size * s * 0.3);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                let r = size * (0.45 + 0.25 * hash(c.id.0, c.id.1, seed + 3));
                if c.d1 > r {
                    return [250.0, 248.0, 242.0];
                }
                let (lx, ly) = lp(cx, c.c.0, c.c.1);
                let p = b.sample(lx, ly, Edge::Clamp);
                let (h, sat, l) = rgb_to_hsl(rgb(p));
                let o = hsl_to_rgb(h, (sat * 1.3).min(1.0), l);
                [o[0] * bright, o[1] * bright, o[2] * bright]
            });
        }
        "ScraperBoard" => {
            let white = e.num("ScraperMode", 0.0) >= 1.0;
            let sp = 2.0 + (40.0 - n(e, "Density", 25.0)).max(0.0) / 6.0;
            let size = n(e, "Size", 5.0).max(1.0) * 2.0;
            *im = build(im, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size * 3.0, seed);
                let a = hash(c.id.0, c.id.1, seed) * PI;
                let v = ex * a.cos() + ey * a.sin();
                let t = luma(&p) / 255.0;
                if ((v / sp).fract() - 0.5).abs() * 2.0 < t * 1.1 {
                    if white {
                        gray3(250.0)
                    } else {
                        rgb(p)
                    }
                } else {
                    gray3(8.0)
                }
            });
        }
        "SketchPad" => {
            let color = e.num("PencilType", 0.0) >= 1.0;
            let style = pct(e, "Style", 25.0) * 2.0;
            let lead = pct(e, "Lead", 75.0);
            let outline = pct(e, "Outline", 25.0) * 2.0;
            let em = edge_mag(im);
            *im = build(im, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let t = 1.0 - luma(&p) / 255.0;
                let rough = (1.0 - style) * (noise(ex * 0.5, ey * 0.5, seed) - 0.5);
                let line = (((ex * 0.8 + ey * 0.6) / 3.0 + rough).fract() - 0.5).abs() * 2.0;
                let mut cover = if line < t * (0.5 + lead) { 0.55 + lead * 0.4 } else { 0.0 };
                cover = (cover + em[y * im.w + x] * outline * 1.5).min(1.0);
                let pencil = if color { [p[0] * 0.55, p[1] * 0.55, p[2] * 0.55] } else { gray3(55.0) };
                lerp3([250.0, 250.0, 247.0], pencil, cover)
            });
        }
        "Watercolor" => {
            let brush = n(e, "BrushSize", 1.0).max(1.0);
            let gran = pct(e, "Granulation", 50.0);
            let water = pct(e, "WaterAmount", 50.0);
            let bleed = pct(e, "Bleed", 35.0);
            let bright = pct(e, "Brightness", 25.0);
            let b = gauss(im, (brush + water * 4.0) * s);
            let em = edge_mag(&b);
            *im = build(&b, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let q = |v: f32| ((v / 255.0 * 8.0).round() / 8.0) * 255.0 * 0.6 + v * 0.4;
                let g = 1.0 - gran * 0.3 * noise(ex * 0.9, ey * 0.9, seed);
                let d = em[y * b.w + x] * bleed * 70.0;
                let c = [q(p[0]) * g - d, q(p[1]) * g - d, q(p[2]) * g - d];
                lerp3(c, [255.0; 3], bright * 0.6)
            });
        }
        "WaterMarker" => {
            let size = n(e, "Size", 1.0).max(1.0) * 3.0 + 3.0;
            let var = pct(e, "ColorVariation", 25.0);
            let mode = ix(e, "VariationMode", 0);
            let b = gauss(im, size * s * 0.3);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                let (lx, ly) = lp(cx, c.c.0, c.c.1);
                let p = b.sample(lx, ly, Edge::Clamp);
                let j = match mode {
                    1 => ((c.id.0 + c.id.1).rem_euclid(5) as f32 / 4.0) - 0.5,
                    2 => hash(c.id.0, c.id.1, seed) - 0.5,
                    _ => (hash(c.id.0, c.id.1, seed) - 0.5) * 0.5,
                };
                let k = 1.0 + j * var * 1.6;
                let rim = if c.d2 - c.d1 < 1.0 { 0.8 } else { 1.0 };
                [p[0] * k * rim, p[1] * k * rim, p[2] * k * rim]
            });
        }
        "WavePaper" => {
            let bw = e.num("ColorMode", 0.0) >= 1.0;
            let pr = n(e, "Pressure", 10.0) / 20.0;
            *im = build(im, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let w = 0.5 + 0.5 * ((ex + 2.0 * (ey / 6.0).sin()) * TAU / 5.0).sin();
                let k = 1.0 - 0.45 * w * pr;
                let c = if bw { gray3(luma(&p)) } else { rgb(p) };
                lerp3([c[0] * k, c[1] * k, c[2] * k], [250.0; 3], (1.0 - w) * 0.15 * pr)
            });
        }
        _ => {}
    }
}

fn keep_alpha(o: &mut Img, src: &Img) {
    for (a, b) in o.p.iter_mut().zip(src.p.iter()) {
        if b[3] <= 0.0 {
            *a = [0.0; 4];
        } else {
            a[3] = b[3];
        }
    }
}

// ------------------------------------------------------------------------------ Camera

fn camera(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let s = cx.s.max(1e-3);
    let seed = cx.seed;
    match e.id.as_str() {
        "Colorize" => {
            let (h, sat) = (n(e, "Hue", 0.0), pct(e, "Saturation", 0.0));
            im.map(|c| {
                let (_, _, l) = rgb_to_hsl(c);
                hsl_to_rgb(h, sat, l)
            });
        }
        "Diffuse" => {
            let r = pct(e, "Level", 50.0) * 4.0;
            let src = im.clone();
            *im = build(&src, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let (ix0, iy0) = (ex.floor() as i64, ey.floor() as i64);
                let dx = (hash(ix0, iy0, seed) - 0.5) * 2.0 * r;
                let dy = (hash(ix0, iy0, seed + 1) - 0.5) * 2.0 * r;
                rgb(src.sample(x as f32 + 0.5 + dx * s, y as f32 + 0.5 + dy * s, Edge::Clamp))
            });
        }
        "PhotoFilter" => {
            let f = col(e.color("ColorItem", Color::rgb(255, 0, 0)));
            let d = pct(e, "Density", 100.0) * 0.6;
            let keep = e.flag("PreserveLuminosity", false);
            im.map(|c| {
                let m = [c[0] * f[0] / 255.0, c[1] * f[1] / 255.0, c[2] * f[2] / 255.0];
                let mut o = lerp3(c, lerp3(m, f, 0.25), d);
                if keep {
                    let k = luma(&[c[0], c[1], c[2], 255.0]) / luma(&[o[0], o[1], o[2], 255.0]).max(1.0);
                    o = [o[0] * k, o[1] * k, o[2] * k];
                }
                o
            });
        }
        "LensFlare" => {
            let (ox, oy) = cx.at(e.num("CenterX", 0.3), e.num("CenterY", 0.3));
            let diag = (cx.fw().powi(2) + cx.fh().powi(2)).sqrt();
            let asp = (pct(e, "AspectRatio", 100.0)).max(0.05);
            let fsize = pct(e, "FlareSize", 10.0) * diag * 0.5 + 2.0;
            let fb = pct(e, "FlareBrightness", 23.0) * 3.0;
            let fall = 1.0 + pct(e, "FlareFallof", 18.0) * 4.0;
            let fc = col(e.color("FlareColorItem", Color::rgb(255, 60, 30)));
            let hsize = pct(e, "HaloSize", 30.0) * diag * 0.5;
            let hb = pct(e, "HaloBrightness", 20.0);
            let hc = col(e.color("HaloColorItem", Color::rgb(255, 200, 120)));
            let rsize = pct(e, "ReflectionSize", 20.0) * diag * 0.08 + 2.0;
            let rb = pct(e, "ReflectionBright", 30.0);
            let rays = n(e, "RayCount", 12.0).max(0.0);
            let rayb = pct(e, "RayBrightness", 30.0);
            let rayrot = n(e, "RayRotation", 0.0).to_radians();
            let rayc = col(e.color("RayColorItem", Color::rgb(255, 230, 200)));
            let (icx, icy) = cx.at(0.5, 0.5);
            let refl: Vec<(f32, f32, f32)> = [0.35f32, 0.6, 0.85, 1.25, 1.6].iter().enumerate().map(|(i, t)| (ox + (icx - ox) * 2.0 * t * 0.6, oy + (icy - oy) * 2.0 * t * 0.6, rsize * (0.6 + i as f32 * 0.35))).collect();
            *im = build(im, |x, y, p| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (dx, dy) = (px - ox, (py - oy) / asp);
                let r = (dx * dx + dy * dy).sqrt();
                let mut add = [0.0f32; 3];
                let core = (-(r / fsize).powf(fall.min(3.0)) * fall).exp() * fb;
                for c in 0..3 {
                    add[c] += (fc[c] * 0.6 + 255.0 * 0.4) * core;
                }
                let ring = (-((r - hsize) / (hsize * 0.08 + 1.0)).powi(2)).exp() * hb;
                for c in 0..3 {
                    add[c] += hc[c] * ring;
                }
                if rays > 0.0 {
                    let a = dy.atan2(dx) + rayrot;
                    let ray = ((a * rays / 2.0).cos().abs().powf(40.0)) * (-(r / (fsize * 6.0))).exp() * rayb;
                    for c in 0..3 {
                        add[c] += rayc[c] * ray;
                    }
                }
                for (i, (rx, ry, rr)) in refl.iter().enumerate() {
                    let d = ((px - rx).powi(2) + (py - ry).powi(2)).sqrt();
                    let k = (1.0 - smooth(rr * 0.8, *rr, d)) * rb * 0.35;
                    let tint = if i % 2 == 0 { fc } else { hc };
                    for c in 0..3 {
                        add[c] += tint[c] * k;
                    }
                }
                [p[0] + add[0], p[1] + add[1], p[2] + add[2]]
            });
        }
        "Lighting" => {
            let (sx, sy) = cx.at(e.num("SourceX", 0.25), e.num("SourceY", 0.25));
            let on = e.flag("EnableLightSource", true);
            let dirl = e.num("LightType", 0.0) >= 1.0;
            let bright = n(e, "Brightness", 100.0) / 100.0;
            let cone = n(e, "ConeSize", 90.0).clamp(1.0, 180.0);
            let edge = pct(e, "Edge", 50.0);
            let lc = col(e.color("LightColorItem", Color::WHITE));
            let atm = if e.flag("EnableAtmosphere", true) { pct(e, "AtmosphereBrightness", 35.0) } else { 0.0 };
            let ac = col(e.color("AtmosphereColorItem", Color::WHITE));
            let expo = 1.0 + pct(e, "Exposure", 0.0);
            let relief = pct(e, "Relief", 0.0);
            let (icx, icy) = cx.at(0.5, 0.5);
            let diag = (cx.fw().powi(2) + cx.fh().powi(2)).sqrt();
            let (spx, spy) = (sx + (icx - sx) * 0.5, sy + (icy - sy) * 0.5);
            let rad = diag * cone / 180.0 * 0.6;
            let ang = (icy - sy).atan2(icx - sx);
            let hm = if relief > 0.0 { Some(shade(&gauss(im, s).luma().iter().map(|v| v / 255.0).collect::<Vec<_>>(), im.w, im.h, (-ang).to_degrees() + 180.0, relief * 6.0)) } else { None };
            *im = build(im, |x, y, p| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut l = [ac[0] / 255.0 * atm, ac[1] / 255.0 * atm, ac[2] / 255.0 * atm];
                if on {
                    let k = if dirl {
                        // a directional light brightens toward the side it comes from
                        let t = ((px - icx) * ang.cos() + (py - icy) * ang.sin()) / diag;
                        (0.75 - t).clamp(0.0, 1.0)
                    } else {
                        let (dx, dy) = (px - spx, py - spy);
                        // the lit area is stretched along the light's direction
                        let (u, v) = (dx * ang.cos() + dy * ang.sin(), -dx * ang.sin() + dy * ang.cos());
                        let r = ((u / 1.4).powi(2) + v * v).sqrt() / rad;
                        1.0 - smooth(1.0 - edge * 0.9 - 0.05, 1.0, r)
                    };
                    for c in 0..3 {
                        l[c] += lc[c] / 255.0 * k * bright;
                    }
                }
                let sh = hm.as_ref().map(|h| h[y * im.w + x]).unwrap_or(1.0);
                [p[0] * l[0] * expo * sh, p[1] * l[1] * expo * sh, p[2] * l[2] * expo * sh]
            });
        }
        "Sepia" => {
            let a = pct(e, "Amount", 50.0);
            im.map(|c| {
                let l = luma(&[c[0], c[1], c[2], 255.0]);
                let sep = [l * 1.07 + 18.0, l * 0.92 + 4.0, l * 0.72 - 6.0];
                lerp3(c, sep, a * 1.6)
            });
        }
        "SpotFilter" => {
            let (ox, oy) = cx.center(e);
            let half = (cx.fw().powi(2) + cx.fh().powi(2)).sqrt() / 2.0;
            let r0 = pct(e, "Sharpenraduis", 40.0) * half;
            let fe = pct(e, "Feathering", 50.0) * half + 1.0;
            let fall = pct(e, "Lightfalloff", 0.0);
            let b = gauss(im, s * 4.0);
            let src = im.clone();
            *im = build(&src, |x, y, p| {
                let d = ((x as f32 + 0.5 - ox).powi(2) + (y as f32 + 0.5 - oy).powi(2)).sqrt();
                let w = smooth(r0, r0 + fe, d);
                let o = lerp3(rgb(p), rgb(b.at(x, y)), w);
                let k = 1.0 - fall * w;
                [o[0] * k, o[1] * k, o[2] * k]
            });
        }
        "TimeMachine" => {
            let style = ix(e, "Style", 0);
            let t = pct(e, "Intensity", 80.0);
            let edges = e.flag("PhotoEdges", true);
            let src = im.clone();
            let soft = if style == 5 { gauss(im, s * 1.5) } else { src.clone() };
            *im = build(&src, |x, y, p| {
                let c = rgb(soft.at(x, y));
                let l = luma(&p) / 255.0;
                let (u, v) = ((x as f32 + 0.5 - cx.frame[0]) / cx.fw(), (y as f32 + 0.5 - cx.frame[1]) / cx.fh());
                let vig = 1.0 - ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 0.9;
                let o: [f32; 3] = match style {
                    // daguerreotype: silvery, high contrast, dark corners
                    0 => {
                        let g = ((l - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0) * vig;
                        [g * 225.0 + 10.0, g * 230.0 + 12.0, g * 235.0 + 18.0]
                    }
                    // albumen: warm brown
                    1 => [l * 240.0 + 20.0, l * 200.0 + 12.0, l * 140.0],
                    // cyanotype: blue
                    2 => [l * 120.0 + 10.0, l * 170.0 + 30.0, l * 210.0 + 45.0],
                    // platinum: warm neutral gray, soft
                    3 => {
                        let g = l * 0.85 + 0.08;
                        [g * 245.0, g * 238.0, g * 225.0]
                    }
                    // early color: faded colors
                    4 => {
                        let (h, sat, ll) = rgb_to_hsl(c);
                        let o = hsl_to_rgb(h + 8.0, sat * 0.55, ll * 0.85 + 0.1);
                        [o[0] + 12.0, o[1] + 5.0, o[2] - 10.0]
                    }
                    // box camera: blurry, warm, vignetted
                    5 => [c[0] * vig * 1.05 + 8.0, c[1] * vig, c[2] * vig * 0.85],
                    // cross process: yellow-green highlights, blue shadows
                    _ => [255.0 * smooth(0.05, 0.95, c[0] / 255.0), 255.0 * (c[1] / 255.0).powf(0.85), 255.0 * (c[2] / 255.0 * 0.7 + 0.15)],
                };
                let mut o = lerp3(rgb(p), o, t);
                if edges {
                    // a rough white border, like an old print
                    let m = u.min(1.0 - u).min(v.min(1.0 - v));
                    let rough = noise(x as f32 / (s * 3.0), y as f32 / (s * 3.0), seed) * 0.012;
                    if m < 0.025 + rough {
                        o = [245.0, 242.0, 235.0];
                    } else if m < 0.04 + rough {
                        o = lerp3(o, [20.0; 3], 0.6);
                    }
                }
                o
            });
        }
        "Halftone" => {
            let angles = [n(e, "CAngle", 15.0), n(e, "MAngle", 75.0), n(e, "YAngle", 0.0), n(e, "KAngle", 45.0)];
            let r = n(e, "MaxRadius", 5.0).max(1.0);
            let cell = r * 2.0;
            let b = gauss(im, r * s * 0.5);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let p = b.at(x, y);
                let (rr, gg, bb) = (p[0] / 255.0, p[1] / 255.0, p[2] / 255.0);
                let k = 1.0 - rr.max(gg).max(bb);
                let d = (1.0 - k).max(1e-3);
                let inks = [(1.0 - rr - k) / d, (1.0 - gg - k) / d, (1.0 - bb - k) / d, k];
                let mut cov = [0.0f32; 4];
                for i in 0..4 {
                    let a = angles[i].to_radians();
                    let (u, v) = (ex * a.cos() + ey * a.sin(), -ex * a.sin() + ey * a.cos());
                    let (fu, fv) = ((u / cell).fract() - 0.5, (v / cell).fract() - 0.5);
                    let fu = if fu < -0.5 { fu + 1.0 } else { fu };
                    let fv = if fv < -0.5 { fv + 1.0 } else { fv };
                    let dist = (fu * fu + fv * fv).sqrt() * cell;
                    let rad = inks[i].clamp(0.0, 1.0).sqrt() * r;
                    cov[i] = 1.0 - smooth(rad - 0.5, rad + 0.5, dist);
                }
                let kk = 1.0 - cov[3];
                [255.0 * (1.0 - cov[0]) * kk, 255.0 * (1.0 - cov[1]) * kk, 255.0 * (1.0 - cov[2]) * kk]
            });
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------ Creative

fn creative(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let s = cx.s.max(1e-3);
    let seed = cx.seed;
    match e.id.as_str() {
        "NNArtStyle" => {
            let style = ix(e, "CurrentStylePreset", 0);
            let t = pct(e, "Intensity", 100.0);
            let detail = ix(e, "Detail", 2) as f32;
            let r = (3.0 - detail) * 1.5 + 1.0;
            let src = im.clone();
            let b = gauss(im, r * s);
            let em = edge_mag(&b);
            let out = build(&src, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let c = rgb(b.at(x, y));
                let m = em[y * src.w + x];
                let o = match style {
                    // smooth acrylic: flat smooth color areas
                    0 => {
                        let q = |v: f32| ((v / 255.0 * 7.0).round() / 7.0) * 255.0 * 0.7 + v * 0.3;
                        [q(c[0]) - m * 30.0, q(c[1]) - m * 30.0, q(c[2]) - m * 30.0]
                    }
                    // graphite: pencil lines
                    1 => {
                        let l = luma(&p) / 255.0;
                        let line = (((ex + ey * 0.3) / 2.5).fract() - 0.5).abs() * 2.0;
                        let v = if line < (1.0 - l) * 0.9 { 70.0 } else { 245.0 } - m * 120.0;
                        gray3(v)
                    }
                    // pastel mosaic
                    2 => {
                        let cl = voronoi(ex, ey, 4.0 + (3.0 - detail) * 2.0, seed);
                        let (lx, ly) = lp(cx, cl.c.0, cl.c.1);
                        let q = b.sample(lx, ly, Edge::Clamp);
                        lerp3(rgb(q), [250.0; 3], 0.25 + if cl.d2 - cl.d1 < 0.8 { 0.3 } else { 0.0 })
                    }
                    // neon: glowing edges on black
                    3 => {
                        let (h, _, _) = rgb_to_hsl(c);
                        let g = hsl_to_rgb(h, 1.0, 0.55);
                        let k = (m * 2.5).min(1.0);
                        [g[0] * k, g[1] * k, g[2] * k]
                    }
                    // woodcut: carved black and paper
                    4 => {
                        let l = luma(&p) / 255.0 + (((ey + noise(ex * 0.1, ey * 0.1, seed) * 6.0) / 3.0).fract() - 0.5) * 0.35;
                        gray3(if l > 0.5 { 238.0 } else { 20.0 })
                    }
                    // lava lamp: warm flowing colors
                    _ => {
                        let l = luma(&p) / 255.0;
                        let f = fbm(ex * 0.02, ey * 0.02, 3, seed);
                        hsl_to_rgb(330.0 + (l + f) * 90.0, 0.85, 0.25 + l * 0.5)
                    }
                };
                o
            });
            im.mix(&out, t);
        }
        "Crystallize" => {
            let size = n(e, "Size", 10.0).max(3.0);
            let b = gauss(im, size * s * 0.25);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                let (lx, ly) = lp(cx, c.c.0, c.c.1);
                rgb(b.sample(lx, ly, Edge::Clamp))
            });
        }
        "Fabric" => {
            let style = ix(e, "Style", 2);
            let size = n(e, "Size", 10.0).max(1.0) + 3.0;
            let complete = pct(e, "Complete", 100.0);
            let bright = 0.5 + pct(e, "Brightness", 50.0);
            let rot = n(e, "Rotation", 0.0).to_radians();
            let b = gauss(im, size * s * 0.3);
            *im = build(im, |x, y, p| {
                let (ex0, ey0) = ep(cx, x, y);
                let (ex, ey) = (ex0 * rot.cos() - ey0 * rot.sin(), ex0 * rot.sin() + ey0 * rot.cos());
                let (gi, gj) = ((ex / size).floor(), (ey / size).floor());
                if hash(gi as i64, gj as i64, seed + 2) > complete {
                    return rgb(p);
                }
                let (cxe, cye) = ((gi + 0.5) * size, (gj + 0.5) * size);
                let (sx0, sy0) = (cxe * rot.cos() + cye * rot.sin(), -cxe * rot.sin() + cye * rot.cos());
                let (lx, ly) = lp(cx, sx0, sy0);
                let c = rgb(b.sample(lx, ly, Edge::Clamp));
                let (fu, fv) = ((ex / size).fract(), (ey / size).fract());
                let pat = match style {
                    // needlepoint: diagonal stitches
                    0 => 0.75 + 0.25 * (((fu + fv) * 4.0 * PI).sin()),
                    // rug hooking: loops
                    1 => 0.7 + 0.3 * (1.0 - ((fu - 0.5).powi(2) + (fv - 0.5).powi(2)).sqrt() * 2.0),
                    // quilt: padded squares with seams
                    2 => {
                        let m = fu.min(1.0 - fu).min(fv.min(1.0 - fv));
                        0.6 + 0.4 * smooth(0.0, 0.25, m)
                    }
                    // strings
                    3 => 0.65 + 0.35 * (fu * 6.0 * PI).sin().abs(),
                    // ribbons
                    4 => 0.7 + 0.3 * (fv * PI).sin(),
                    // tissue collage: overlapping translucent pieces
                    _ => 0.8 + 0.2 * hash(gi as i64, gj as i64, seed),
                };
                [c[0] * pat * bright, c[1] * pat * bright, c[2] * pat * bright]
            });
        }
        "Frame" => frame(e, im, cx),
        "GlassBlock" => {
            let (bw, bh) = (n(e, "BlockWidth", 20.0).max(1.0), n(e, "BlockHeight", 20.0).max(1.0));
            *im = im.remap(Edge::Clamp, |x, y| {
                let (ex, ey) = ((x - cx.frame[0]) / s, (y - cx.frame[1]) / s);
                let (cxe, cye) = (((ex / bw).floor() + 0.5) * bw, ((ey / bh).floor() + 0.5) * bh);
                // each block shows a magnified, shifted piece of the image
                let (sx0, sy0) = (cxe + (ex - cxe) * 0.45 + bw * 0.15, cye + (ey - cye) * 0.45 + bh * 0.15);
                lp(cx, sx0, sy0)
            });
        }
        "Mosaic" => {
            let size = n(e, "Size", 10.0).max(2.0) + 2.0;
            let bg = col(e.color("BackgroundColorItem", Color::rgb(189, 183, 107)));
            let vig = e.flag("Vignette", false);
            let b = gauss(im, size * s * 0.3);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                if c.d2 - c.d1 < 1.3 {
                    return bg;
                }
                let (lx, ly) = lp(cx, c.c.0, c.c.1);
                let p = rgb(b.sample(lx, ly, Edge::Clamp));
                let k = if vig {
                    let (u, v) = ((x as f32 - cx.frame[0]) / cx.fw() - 0.5, (y as f32 - cx.frame[1]) / cx.fh() - 0.5);
                    1.0 - (u * u + v * v).sqrt() * 0.9
                } else {
                    1.0
                };
                [p[0] * k, p[1] * k, p[2] * k]
            });
        }
        "Scatter" => {
            let (hx, vy) = (n(e, "Horizontal", 10.0), n(e, "Vertical", 10.0));
            *im = im.remap(Edge::Clamp, |x, y| {
                let (ex, ey) = (((x - cx.frame[0]) / s).floor() as i64, ((y - cx.frame[1]) / s).floor() as i64);
                (x + (hash(ex, ey, seed) - 0.5) * 2.0 * hx * s, y + (hash(ex, ey, seed + 1) - 0.5) * 2.0 * vy * s)
            });
        }
        "SmokedGlass" => {
            let tint = pct(e, "Tint", 40.0);
            let blur = pct(e, "Level", 90.0);
            let c = col(e.color("ColorItem", Color::BLACK));
            let b = gauss(im, blur * 3.0 * s);
            *im = build(&b, |_, _, p| lerp3(rgb(p), c, tint));
        }
        "StainedGlass" => {
            let size = n(e, "Size", 20.0).max(3.0);
            let light = n(e, "LightIntensity", 2.0);
            let th = n(e, "Thickness", 10.0).max(1.0) * 0.2;
            let solder = col(e.color("BorderColorItem", Color::BLACK));
            let lit3d = e.flag("3DLighting", false);
            let b = gauss(im, size * s * 0.25);
            *im = build(im, |x, y, _| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, size, seed);
                let edge = (c.d2 - c.d1) / 2.0;
                if edge < th {
                    return solder;
                }
                let (lx, ly) = lp(cx, c.c.0, c.c.1);
                let p = rgb(b.sample(lx, ly, Edge::Clamp));
                let mut k = 0.85 + light * 0.08;
                if lit3d {
                    k *= 0.8 + 0.4 * smooth(th, th + 3.0, edge);
                }
                [p[0] * k, p[1] * k, p[2] * k]
            });
        }
        "Vignette" => {
            let c = match ix(e, "ColorMode", 0) {
                0 => [0.0; 3],
                1 => [255.0; 3],
                _ => col(e.color("ColorItem", Color::WHITE)),
            };
            let shape = ix(e, "Shape", 0);
            let off = n(e, "Offset", 100.0) / 100.0;
            let fade = pct(e, "Fade", 75.0).max(0.01);
            let (fw, fh) = (cx.fw(), cx.fh());
            *im = build(im, |x, y, p| {
                let (u, v) = ((x as f32 + 0.5 - cx.frame[0]) / fw * 2.0 - 1.0, (y as f32 + 0.5 - cx.frame[1]) / fh * 2.0 - 1.0);
                let d = match shape {
                    0 => (u * u + v * v).sqrt(),
                    1 => {
                        let m = fw.min(fh);
                        ((u * fw / m).powi(2) + (v * fh / m).powi(2)).sqrt()
                    }
                    2 => u.abs().max(v.abs()),
                    _ => {
                        let m = fw.min(fh);
                        (u * fw / m).abs().max((v * fh / m).abs())
                    }
                };
                // Offset moves the edge of the clear area out; Fade widens the blend
                let inner = off * 0.75;
                let t = smooth(inner, inner + fade * 0.8, d);
                lerp3(rgb(p), c, t)
            });
        }
        "Vortex" => {
            let (ox, oy) = cx.center(e);
            let style = ix(e, "Style", 0);
            let size = n(e, "Size", 1.0).max(1.0);
            let (inner, outer) = (n(e, "InnerDirection", 180.0).to_radians(), n(e, "OuterDirection", 180.0).to_radians());
            let rmax = (cx.fw().powi(2) + cx.fh().powi(2)).sqrt() / 2.0;
            let len = size * s * match style {
                2 => 10.0,
                3 => 3.0,
                _ => 6.0,
            };
            let src = im.clone();
            *im = build(&src, |x, y, _| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (dx, dy) = (px - ox, py - oy);
                let r = (dx * dx + dy * dy).sqrt();
                let a = dy.atan2(dx) + PI / 2.0 + inner + (outer - inner) * (r / rmax).min(1.0) - PI;
                let (sx0, sy0) = (a.cos() * len / 6.0, a.sin() * len / 6.0);
                let mut acc = [0.0f32; 3];
                for k in -3..=3 {
                    let q = src.sample(px + sx0 * k as f32, py + sy0 * k as f32, Edge::Clamp);
                    for c in 0..3 {
                        acc[c] += q[c] / 7.0;
                    }
                }
                if style == 1 {
                    let q = |v: f32| ((v / 255.0 * 5.0).round() / 5.0) * 255.0;
                    acc = [q(acc[0]), q(acc[1]), q(acc[2])];
                }
                acc
            });
        }
        _ => {}
    }
}

fn frame(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let kind = ix(e, "CurrentFrameImage", 0);
    let (hs, vs) = (pct(e, "Horizontal", 100.0), pct(e, "Vertical", 100.0));
    let rot = n(e, "Rotate", 0.0).to_radians();
    let c = col(e.color("ColorItem", Color::BLACK));
    let op = pct(e, "Opacity", 100.0);
    let blur = pct(e, "BlurFeather", 0.0);
    let (hf, vf) = (e.flag("HFlip", false), e.flag("VFlip", false));
    let (ocx, ocy) = cx.at(0.5, 0.5);
    let (fw, fh) = (cx.fw() * hs, cx.fh() * vs);
    let seed = cx.seed;
    let s = cx.s.max(1e-3);
    // the mask of the picture area (1 inside, 0 where the frame covers it)
    let mut m = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            let (dx, dy) = (x as f32 + 0.5 - ocx, y as f32 + 0.5 - ocy);
            let (mut u, mut v) = (dx * rot.cos() + dy * rot.sin(), -dx * rot.sin() + dy * rot.cos());
            if hf {
                u = -u;
            }
            if vf {
                v = -v;
            }
            let (nu, nv) = (u / (fw / 2.0).max(1.0), v / (fh / 2.0).max(1.0));
            // distance to the frame's edge, in effect px (positive inside)
            let d = (1.0 - nu.abs()).min(1.0 - nv.abs()) * fw.min(fh) / 2.0 / s;
            let (ex, ey) = (u / s, v / s);
            let border = match kind {
                0 => 8.0,
                1 => 6.0 + (noise(ex * 0.15, ey * 0.15, seed) - 0.5) * 8.0,
                2 => 7.0 + (fbm(ex * 0.08, ey * 0.08, 4, seed) - 0.5) * 18.0,
                3 => 9.0 + (noise(ex * 0.6, ey * 0.04, seed) + noise(ex * 0.04, ey * 0.6, seed + 1) - 1.0) * 10.0,
                4 => {
                    let r = fw.min(fh) / s * 0.12;
                    let (ax, ay) = ((nu.abs() * fw / 2.0 / s - (fw / 2.0 / s - r - 6.0)).max(0.0), (nv.abs() * fh / 2.0 / s - (fh / 2.0 / s - r - 6.0)).max(0.0));
                    let k = (ax * ax + ay * ay).sqrt() - r;
                    if k > 0.0 {
                        1e9
                    } else {
                        6.0
                    }
                }
                _ => {
                    // film: a black border with sprocket holes
                    let hole = ((ex / 10.0).fract() - 0.5).abs() < 0.2 && (d - 4.0).abs() < 2.5;
                    if hole {
                        -1.0
                    } else {
                        10.0
                    }
                }
            };
            let soft = if kind == 0 { 10.0 } else { 1.0 };
            let inside = if border < 0.0 { 1.0 } else { smooth(border, border + soft, d) };
            let inside = if kind == 5 && border < 0.0 { 1.0 } else { inside };
            m.p[y * im.w + x] = [inside * 255.0, 0.0, 0.0, 255.0];
        }
    }
    if blur > 0.0 {
        m = gauss(&m, blur * 12.0 * s);
    }
    for (p, k) in im.p.iter_mut().zip(m.p.iter()) {
        if p[3] <= 0.0 {
            continue;
        }
        let t = (1.0 - k[0] / 255.0) * op;
        for ch in 0..3 {
            p[ch] = p[ch] * (1.0 - t) + c[ch] * t;
        }
    }
}

// ------------------------------------------------------------------------------ Distort

fn distort(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let s = cx.s.max(1e-3);
    let seed = cx.seed;
    let (fx0, fy0) = (cx.frame[0], cx.frame[1]);
    match e.id.as_str() {
        "Blocks" => {
            let (bw, bh) = (n(e, "Width", 10.0).max(1.0), n(e, "Height", 10.0).max(1.0));
            let off = pct(e, "Offset", 50.0);
            let area = ix(e, "UndefinedArea", 4);
            let other = col(e.color("ColorItem", Color::WHITE));
            let src = im.clone();
            let mut out = Img::new(im.w, im.h);
            // the undefined areas first, then the blocks moved over them
            for (i, p) in src.p.iter().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                out.p[i] = match area {
                    0 => *p,
                    1 => [255.0 - p[0], 255.0 - p[1], 255.0 - p[2], p[3]],
                    2 => [0.0, 0.0, 0.0, p[3]],
                    3 => [255.0, 255.0, 255.0, p[3]],
                    _ => [other[0], other[1], other[2], p[3]],
                };
            }
            for y in 0..im.h {
                for x in 0..im.w {
                    let (ex, ey) = ep(cx, x, y);
                    let (gi, gj) = ((ex / bw).floor() as i64, (ey / bh).floor() as i64);
                    // a block's pixel lands shifted by the block's random offset
                    let (ox, oy) = ((hash(gi, gj, seed) - 0.5) * bw * off, (hash(gi, gj, seed + 1) - 0.5) * bh * off);
                    let (tx, ty) = (x as f32 + ox * s, y as f32 + oy * s);
                    if tx < 0.0 || ty < 0.0 || tx >= im.w as f32 || ty >= im.h as f32 {
                        continue;
                    }
                    let p = src.p[y * im.w + x];
                    if p[3] > 0.0 {
                        out.p[ty as usize * im.w + tx as usize] = p;
                    }
                }
            }
            *im = out;
        }
        "Displace" => {
            let stretch = e.num("ScaleMode", 0.0) >= 1.0;
            let (hsc, vsc) = (n(e, "HorizontalScale", 10.0), n(e, "VerticalScale", 10.0));
            let wrap = e.num("UndefinedArea", 0.0) >= 1.0;
            let kind = ix(e, "CurrentDisplaceImage", 0);
            let (fw, fh) = (cx.fw() / s, cx.fh() / s);
            *im = im.remap(if wrap { Edge::Wrap } else { Edge::Clamp }, |x, y| {
                let (ex, ey) = ((x - fx0) / s, (y - fy0) / s);
                let (u, v) = if stretch { (ex / fw.max(1.0) * 64.0, ey / fh.max(1.0) * 64.0) } else { (ex, ey) };
                let dx = (texture(kind, u, v, seed) - 0.5) * 2.0 * hsc;
                let dy = (texture(kind, v + 31.0, u + 17.0, seed) - 0.5) * 2.0 * vsc;
                (x + dx * s, y + dy * s)
            });
        }
        "MeshWarp" => {
            // the moved nodes: a list of (u, v) per node, row by row, in 0..1 of the object
            let g = (n(e, "Gridlines", 4.0).max(2.0)) as usize;
            let Some(mesh) = e.list("Mesh") else { return };
            if mesh.len() != (g + 1) * (g + 1) * 2 {
                return;
            }
            let node = |i: usize, j: usize| (mesh[(j * (g + 1) + i) * 2] as f32, mesh[(j * (g + 1) + i) * 2 + 1] as f32);
            let (fw, fh) = (cx.fw(), cx.fh());
            // for each output pixel find the cell it falls in by inverting the bilinear map
            // (approximately: the moved mesh's cells are searched)
            let src = im.clone();
            let mut out = Img::new(im.w, im.h);
            for j in 0..g {
                for i in 0..g {
                    let q = [node(i, j), node(i + 1, j), node(i + 1, j + 1), node(i, j + 1)];
                    let to_px = |p: (f32, f32)| (fx0 + p.0 * fw, fy0 + p.1 * fh);
                    let qp: Vec<(f32, f32)> = q.iter().map(|p| to_px(*p)).collect();
                    let (minx, maxx) = (qp.iter().map(|p| p.0).fold(f32::MAX, f32::min), qp.iter().map(|p| p.0).fold(f32::MIN, f32::max));
                    let (miny, maxy) = (qp.iter().map(|p| p.1).fold(f32::MAX, f32::min), qp.iter().map(|p| p.1).fold(f32::MIN, f32::max));
                    let steps = ((maxx - minx).max(maxy - miny) * 1.5).ceil().max(2.0) as usize;
                    for b in 0..=steps {
                        for a in 0..=steps {
                            let (sa, sb) = (a as f32 / steps as f32, b as f32 / steps as f32);
                            let top = (qp[0].0 + (qp[1].0 - qp[0].0) * sa, qp[0].1 + (qp[1].1 - qp[0].1) * sa);
                            let bot = (qp[3].0 + (qp[2].0 - qp[3].0) * sa, qp[3].1 + (qp[2].1 - qp[3].1) * sa);
                            let (px, py) = (top.0 + (bot.0 - top.0) * sb, top.1 + (bot.1 - top.1) * sb);
                            if px < 0.0 || py < 0.0 || px >= im.w as f32 || py >= im.h as f32 {
                                continue;
                            }
                            let (su, sv) = ((i as f32 + sa) / g as f32, (j as f32 + sb) / g as f32);
                            out.p[py as usize * im.w + px as usize] = src.sample(fx0 + su * fw, fy0 + sv * fh, Edge::Clear);
                        }
                    }
                }
            }
            *im = out;
        }
        "Offset" => {
            let pctm = e.flag("AsPercent", true);
            let (h, v) = (n(e, "Horizontal", 50.0), n(e, "Vertical", 50.0));
            let (dx, dy) = if pctm { (h / 100.0 * cx.fw(), v / 100.0 * cx.fh()) } else { (h * s, v * s) };
            let area = ix(e, "UndefinedArea", 0);
            let other = col(e.color("ColorItem", Color::WHITE));
            let (fw, fh) = (cx.fw(), cx.fh());
            let src = im.clone();
            *im = build(&src, |x, y, _| {
                let (mut sx, mut sy) = (x as f32 + 0.5 - dx - fx0, y as f32 + 0.5 - dy - fy0);
                match area {
                    0 => {
                        sx = sx.rem_euclid(fw);
                        sy = sy.rem_euclid(fh);
                    }
                    1 => {
                        sx = sx.clamp(0.0, fw - 0.5);
                        sy = sy.clamp(0.0, fh - 0.5);
                    }
                    _ => {
                        if sx < 0.0 || sy < 0.0 || sx > fw || sy > fh {
                            return other;
                        }
                    }
                }
                rgb(src.sample(sx + fx0, sy + fy0, Edge::Clamp))
            });
        }
        "Pixelate" => {
            let mode = ix(e, "Mode", 0);
            let (mut pw, ph) = (n(e, "Width", 10.0).max(1.0), n(e, "Height", 10.0).max(1.0));
            let ph = if mode == 0 { pw } else { ph };
            if mode == 0 {
                pw = pw.max(1.0);
            }
            let op = pct(e, "Opacity", 100.0);
            let (ox, oy) = cx.center(e);
            let b = box_blur(im, (pw * s / 2.0) as usize, (ph * s / 2.0) as usize);
            let src = im.clone();
            *im = build(&src, |x, y, p| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (sx, sy) = if mode == 2 {
                    // radial: rings and sectors around the center
                    let (dx, dy) = (px - ox, py - oy);
                    let r = (dx * dx + dy * dy).sqrt();
                    let a = dy.atan2(dx);
                    let rr = ((r / (pw * s)).floor() + 0.5) * pw * s;
                    let step = (ph * s / rr.max(1.0)).min(PI);
                    let aa = ((a / step).floor() + 0.5) * step;
                    (ox + rr * aa.cos(), oy + rr * aa.sin())
                } else {
                    let (ex, ey) = ((px - fx0) / s, (py - fy0) / s);
                    (fx0 + ((ex / pw).floor() + 0.5) * pw * s, fy0 + ((ey / ph).floor() + 0.5) * ph * s)
                };
                lerp3(rgb(p), rgb(b.sample(sx, sy, Edge::Clamp)), op)
            });
        }
        "Ripple" => {
            let period = n(e, "Period", 20.0).max(1.0);
            let amp = n(e, "Amplitude", 20.0) * 0.25;
            let perp = e.flag("PerpendicularWave", false);
            let pamp = n(e, "PerpAmplitude", 20.0) * 0.25;
            let ang = n(e, "Angle", 90.0).to_radians();
            let dist = e.flag("DistortRipple", false);
            let (ca, sa) = (ang.cos(), -ang.sin());
            *im = im.remap(Edge::Clamp, |x, y| {
                let (ex, ey) = ((x - fx0) / s, (y - fy0) / s);
                // along the wave's direction, the image moves across it
                let t = ex * ca + ey * sa;
                let mut ph = t / period * TAU;
                if dist {
                    ph += (t / (period * 3.7)).sin() * 2.0;
                }
                let d = amp * ph.sin();
                let (mut sx, mut sy) = (ex - sa * d, ey + ca * d);
                if perp {
                    let u = -ex * sa + ey * ca;
                    let d2 = pamp * (u / period * TAU).sin();
                    sx += ca * d2;
                    sy += sa * d2;
                }
                lp(cx, sx, sy)
            });
        }
        "Shear" => {
            let vert = e.num("Orientation", 1.0) >= 1.0;
            let scale = pct(e, "Scale", 50.0);
            let edge = ix(e, "EdgeStyle", 2);
            let other = col(e.color("ColorItem", Color::WHITE));
            // the shear curve: (position along, offset) pairs in 0..1, offset 0.5 = no shift
            let pts: Vec<(f32, f32)> = e.list("Curve").map(|l| l.chunks_exact(2).map(|p| (p[0] as f32 * 255.0, p[1] as f32 * 255.0)).collect()).unwrap_or_else(|| vec![(0.0, 127.5), (255.0, 127.5)]);
            let lut = curve_lut(&pts);
            let (fw, fh) = (cx.fw(), cx.fh());
            let src = im.clone();
            *im = build(&src, |x, y, _| {
                let (px, py) = (x as f32 + 0.5 - fx0, y as f32 + 0.5 - fy0);
                let (along, len) = if vert { (py / fh, fw) } else { (px / fw, fh) };
                let k = lut[(along.clamp(0.0, 1.0) * 255.0) as usize] / 255.0 - 0.5;
                let shift = k * len * scale * 2.0;
                let (mut sx, mut sy) = if vert { (px - shift, py) } else { (px, py - shift) };
                match edge {
                    0 => {
                        sx = sx.rem_euclid(fw);
                        sy = sy.rem_euclid(fh);
                    }
                    1 => {
                        sx = sx.clamp(0.0, fw - 0.5);
                        sy = sy.clamp(0.0, fh - 0.5);
                    }
                    _ => {
                        if sx < 0.0 || sy < 0.0 || sx > fw || sy > fh {
                            return other;
                        }
                    }
                }
                rgb(src.sample(sx + fx0, sy + fy0, Edge::Clamp))
            });
        }
        "Swirl" => {
            let (ox, oy) = cx.center(e);
            let cw = e.num("Direction", 0.0) < 1.0;
            let total = (n(e, "Rotation", 0.0) * 360.0 + n(e, "Degree", 90.0)).to_radians() * if cw { 1.0 } else { -1.0 };
            let rmax = (cx.fw().powi(2) + cx.fh().powi(2)).sqrt() / 2.0;
            *im = im.remap(Edge::Clear, |x, y| {
                let (dx, dy) = (x - ox, y - oy);
                let r = (dx * dx + dy * dy).sqrt();
                if r >= rmax {
                    return (x, y);
                }
                let a = -total * (1.0 - r / rmax).powi(2);
                let (sn, cs) = a.sin_cos();
                (ox + dx * cs - dy * sn, oy + dx * sn + dy * cs)
            });
        }
        "Tile" => {
            let (tr, tc) = (n(e, "TilesPerRow", 10.0).max(1.0), n(e, "TilesPerCol", 10.0).max(1.0));
            let ov = pct(e, "PercentOverlap", 25.0);
            let (fw, fh) = (cx.fw(), cx.fh());
            let src = im.clone();
            *im = build(&src, |x, y, _| {
                let (u, v) = ((x as f32 + 0.5 - fx0) / fw * tr, (y as f32 + 0.5 - fy0) / fh * tc);
                // each tile is the whole image, a little larger so neighbours overlap
                let (fu, fv) = (u.fract(), v.fract());
                let k = 1.0 + ov;
                let (su, sv) = ((fu - 0.5) / k + 0.5, (fv - 0.5) / k + 0.5);
                rgb(src.sample(fx0 + su * fw, fy0 + sv * fh, Edge::Clamp))
            });
        }
        "WetPaint" => {
            let wet = n(e, "Wetness", 45.0) / 50.0;
            let perc = pct(e, "Percent", 100.0);
            let l = im.luma();
            let src = im.clone();
            let w = im.w;
            *im = build(&src, |x, y, p| {
                let (ex, _) = ep(cx, x, y);
                let colh = hash(ex.floor() as i64, 0, seed);
                let len = (colh * 18.0 * perc * wet.abs() * s) as usize;
                // drips: a light (or dark) color above runs down over this pixel
                let mut best = p;
                let mut bl = l[y * w + x];
                for k in 1..=len.min(y) {
                    let i = (y - k) * w + x;
                    let v = l[i];
                    if (wet > 0.0 && v > bl + 10.0) || (wet < 0.0 && v < bl - 10.0) {
                        best = src.p[i];
                        bl = v;
                    }
                }
                rgb(best)
            });
        }
        "Whirlpool" => {
            let spacing = n(e, "Spacing", 20.0).max(5.0);
            let warp = e.flag("Warp", true);
            let smear = n(e, "SmearLength", 9.0).max(1.0);
            let twist = n(e, "Twist", 70.0).to_radians();
            let detail = pct(e, "StreakDetail", 60.0);
            let src = im.clone();
            let flow = |ex: f32, ey: f32| {
                // the flow turns around whirl centers placed on a jittered grid
                let c = voronoi(ex, ey, spacing, seed);
                let (dx, dy) = (ex - c.c.0, ey - c.c.1);
                let a = dy.atan2(dx) + PI / 2.0 + twist * (1.0 - (c.d1 / spacing).min(1.0));
                (a.cos(), a.sin())
            };
            *im = build(&src, |x, y, _| {
                let (mut ex, mut ey) = ep(cx, x, y);
                if warp {
                    let (fx, fy) = flow(ex, ey);
                    ex += fx * spacing * 0.15;
                    ey += fy * spacing * 0.15;
                }
                let mut acc = [0.0f32; 3];
                let n = 9;
                let (mut qx, mut qy) = (ex, ey);
                for _ in 0..n {
                    let (lx, ly) = lp(cx, qx, qy);
                    let q = src.sample(lx, ly, Edge::Clamp);
                    for c in 0..3 {
                        acc[c] += q[c] / n as f32;
                    }
                    let (fx, fy) = flow(qx, qy);
                    qx += fx * smear / n as f32;
                    qy += fy * smear / n as f32;
                }
                let grain = 1.0 + (noise(ex * 0.8, ey * 0.8, seed) - 0.5) * detail * 0.3;
                [acc[0] * grain, acc[1] * grain, acc[2] * grain]
            });
        }
        "Wind" => {
            let strength = pct(e, "Strength", 75.0);
            let op = pct(e, "Opacity", 100.0);
            let ang = n(e, "Angle", 0.0).to_radians();
            let len = strength * 30.0 * s;
            let steps = (len.ceil() as usize).clamp(1, 60);
            let (dx, dy) = (-ang.cos() * len / steps as f32, ang.sin() * len / steps as f32);
            let src = im.clone();
            *im = build(&src, |x, y, p| {
                // light colors upwind blow over this pixel and fade with distance
                let mut best = rgb(p);
                let mut bl = luma(&p);
                let (ex, ey) = ep(cx, x, y);
                let streak = 0.6 + 0.4 * hash((ey * ang.cos() + ex * ang.sin()).floor() as i64, 3, seed);
                for k in 1..=steps {
                    let q = src.sample(x as f32 + 0.5 + dx * k as f32, y as f32 + 0.5 + dy * k as f32, Edge::Clamp);
                    let fade = (1.0 - k as f32 / steps as f32) * streak;
                    let v = luma(&q) * fade + bl * (1.0 - fade);
                    if v > bl {
                        best = lerp3(best, rgb(q), fade);
                        bl = v;
                    }
                }
                lerp3(rgb(p), best, op)
            });
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------ Texture

/// Shades the image with a height map (0..1 per pixel) lit from `from` degrees.
fn relief(im: &mut Img, hm: &[f32], from: f32, depth: f32, mix_color: Option<[f32; 3]>, amount: f32) {
    let sh = shade(hm, im.w, im.h, from, depth);
    for (i, p) in im.p.iter_mut().enumerate() {
        if p[3] <= 0.0 {
            continue;
        }
        let base = match mix_color {
            Some(c) => lerp3([p[0], p[1], p[2]], c, amount),
            None => [p[0], p[1], p[2]],
        };
        for c in 0..3 {
            p[c] = clamp255(base[c] * sh[i]);
        }
    }
}

/// The direction the light comes from for a reference "light direction" (it gives where the light
/// travels to: 315° is light from the top left).
fn from_dir(d: f32) -> f32 {
    d + 180.0
}

fn hmap(im: &Img, cx: &FxCtx, f: impl Fn(f32, f32) -> f32) -> Vec<f32> {
    let mut v = vec![0.0; im.w * im.h];
    for y in 0..im.h {
        for x in 0..im.w {
            let (ex, ey) = ep(cx, x, y);
            v[y * im.w + x] = f(ex, ey);
        }
    }
    v
}

fn texture_fx(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) {
    let s = cx.s.max(1e-3);
    let seed = cx.seed;
    // heights are per effect pixel; shading works on layer pixels, so the slope is scaled back
    let k = 1.0 / s;
    match e.id.as_str() {
        "BrickWall" => {
            let rough = pct(e, "Roughness", 25.0);
            let bw = n(e, "BrickWidth", 60.0).max(2.0);
            let bh = n(e, "BrickHeight", 20.0).max(2.0);
            let gw = n(e, "GroutWidth", 5.0).max(0.0);
            let dir = from_dir(n(e, "LightDirection", 315.0));
            let hm = hmap(im, cx, |ex, ey| {
                let row = (ey / bh).floor();
                let u = ex + if (row as i64).rem_euclid(2) == 1 { bw / 2.0 } else { 0.0 };
                let (fx, fy) = ((u / bw).fract() * bw, (ey / bh).fract() * bh);
                let m = fx.min(bw - fx).min(fy.min(bh - fy));
                let brick = smooth(gw / 2.0, gw / 2.0 + 2.0, m);
                brick * 6.0 + rough * fbm(ex * 0.3, ey * 0.3, 3, seed) * 4.0
            });
            relief(im, &hm, dir, k * 0.6, None, 0.0);
            // the grout is lighter mortar
            for (p, h) in im.p.iter_mut().zip(hm.iter()) {
                if p[3] > 0.0 && *h < 1.0 {
                    for c in 0..3 {
                        p[c] = p[c] * 0.5 + 100.0;
                    }
                }
            }
        }
        "Bubbles" => {
            let dia = n(e, "Diameter", 50.0).max(1.0) * 0.4 + 2.0;
            let cover = pct(e, "Coverage", 50.0);
            let dir = from_dir(n(e, "LightDirection", 315.0)).to_radians();
            let refr = e.flag("Refraction", true);
            let src = im.clone();
            let (lx, ly) = (dir.cos(), -dir.sin());
            *im = build(&src, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let c = voronoi(ex, ey, dia, seed);
                if hash(c.id.0, c.id.1, seed + 4) > cover {
                    return rgb(p);
                }
                let r = dia * (0.3 + 0.2 * hash(c.id.0, c.id.1, seed + 8));
                if c.d1 > r {
                    return rgb(p);
                }
                let (dx, dy) = ((ex - c.c.0) / r, (ey - c.c.1) / r);
                let z = (1.0 - dx * dx - dy * dy).max(0.0).sqrt();
                let q = if refr {
                    let (qx, qy) = lp(cx, ex - dx * (1.0 - z) * r * 0.6, ey - dy * (1.0 - z) * r * 0.6);
                    rgb(src.sample(qx, qy, Edge::Clamp))
                } else {
                    rgb(p)
                };
                let spec = (dx * lx + dy * ly).max(0.0).powi(8) * (1.0 - z).powf(0.5) * 255.0;
                let rim = smooth(0.85, 1.0, (dx * dx + dy * dy).sqrt()) * 60.0;
                [q[0] + spec + rim, q[1] + spec + rim, q[2] + spec + rim]
            });
        }
        "Canvas" => {
            let trans = pct(e, "Transparency", 100.0);
            let emb = n(e, "Emboss", 100.0) / 100.0;
            let (ox, oy) = (n(e, "XOffset", 0.0), n(e, "YOffset", 0.0));
            let mode = ix(e, "Mode", 0);
            let toff = pct(e, "TileOffset", 0.0);
            let kind = ix(e, "CurrentCanvasImage", 0);
            let (fw, fh) = (cx.fw() / s, cx.fh() / s);
            let hm = hmap(im, cx, |ex, ey| {
                let (mut u, mut v) = (ex + ox, ey + oy);
                match mode {
                    0 => u += ((v / 64.0).floor()) * toff * 64.0,
                    1 => v += ((u / 64.0).floor()) * toff * 64.0,
                    _ => {
                        u = u / fw.max(1.0) * 128.0;
                        v = v / fh.max(1.0) * 128.0;
                    }
                }
                texture(kind, u, v, seed) * 4.0
            });
            let mut tex = im.clone();
            relief(&mut tex, &hm, 135.0, k * emb, None, 0.0);
            // Transparency is how much of the image shows through the canvas
            for (i, p) in im.p.iter_mut().enumerate() {
                let canvas = 235.0 * (1.0 + (hm[i] / 4.0 - 0.5) * 0.25);
                for c in 0..3 {
                    p[c] = clamp255(tex.p[i][c] * trans + canvas * (1.0 - trans) * (tex.p[i][c] / p[c].max(1.0)).min(1.5));
                }
            }
        }
        "CobbleStone" => {
            let rough = pct(e, "Roughness", 10.0);
            let size = n(e, "Size", 20.0).max(2.0);
            let gw = n(e, "GroutWidth", 10.0) * 0.2;
            let dir = from_dir(n(e, "Direction", 315.0));
            let warp = e.flag("Warp", false);
            let hm = hmap(im, cx, |ex, ey| {
                let (u, v) = if warp { (ex + (noise(ex * 0.05, ey * 0.05, seed) - 0.5) * size, ey + (noise(ey * 0.05, ex * 0.05, seed + 1) - 0.5) * size) } else { (ex, ey) };
                let c = voronoi(u, v, size, seed);
                let edge = (c.d2 - c.d1) / 2.0;
                smooth(gw, gw + size * 0.3, edge) * 6.0 + rough * fbm(ex * 0.4, ey * 0.4, 3, seed) * 5.0
            });
            relief(im, &hm, dir, k * 0.7, None, 0.0);
        }
        "ElephantSkin" => {
            let age = pct(e, "Age", 20.0);
            let sd = seed ^ (e.num("Variation", 1.0) as u32).wrapping_mul(2654435761);
            let c = col(e.color("ColorItem", Color::BLACK));
            *im = build(im, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                // wrinkles: the ridges of a fractal noise
                let f = fbm(ex * 0.03, ey * 0.03, 4, sd);
                let ridge = 1.0 - ((f - 0.5).abs() * 2.0);
                let w = smooth(0.85 - age * 0.25, 1.0, ridge) * (0.4 + age);
                lerp3(rgb(p), c, w * 0.8)
            });
        }
        "Etching" | "ReliefSculpture" => {
            let etch = e.id == "Etching";
            let detail = pct(e, "Detail", 85.0);
            let depth = pct(e, "Depth", if etch { 50.0 } else { 60.0 });
            let sm = pct(e, "Smoothness", 15.0);
            let dir = from_dir(n(e, "LightDirection", 315.0));
            let surf = col(e.color("SurfaceColorItem", Color::rgb(128, 128, 128)));
            let b = gauss(im, (1.0 - detail) * 4.0 * s + sm * 3.0 * s);
            let hm: Vec<f32> = b.luma().iter().map(|v| v / 255.0 * 8.0).collect();
            let sh = shade(&hm, im.w, im.h, dir, depth * 1.5);
            let em = if etch { edge_mag(&b) } else { vec![0.0; im.w * im.h] };
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                let v = sh[i];
                for c in 0..3 {
                    p[c] = clamp255(surf[c] * v - em[i] * 60.0 + if etch { (v - 1.0) * 40.0 } else { 0.0 });
                }
            }
        }
        "Plastic" => {
            let hi = pct(e, "Highlight", 90.0);
            let depth = pct(e, "Depth", 10.0);
            let sm = pct(e, "Smoothness", 30.0);
            let dir = from_dir(n(e, "LightDirection", 315.0));
            let lc = col(e.color("LightColorItem", Color::WHITE));
            let b = gauss(im, (1.0 + sm * 6.0) * s);
            let hm: Vec<f32> = b.luma().iter().map(|v| v / 255.0 * 10.0).collect();
            let sh = shade(&hm, im.w, im.h, dir, depth * 6.0);
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                let spec = ((sh[i] - 1.0).max(0.0) * 3.0).powf(1.5) * hi;
                let q = b.p[i];
                for c in 0..3 {
                    p[c] = clamp255(q[c] * sh[i].min(1.0) + lc[c] * spec);
                }
            }
        }
        "PlasterWall" => {
            let detail = pct(e, "Detail", 50.0);
            let bright = pct(e, "Brightness", 50.0);
            let sd = seed ^ (e.num("Randomize", 1.0) as u32).wrapping_mul(2246822519);
            let hm = hmap(im, cx, |ex, ey| fbm(ex * (0.03 + detail * 0.1), ey * (0.03 + detail * 0.1), 4, sd) * 6.0);
            let b = gauss(im, 2.0 * s);
            *im = b;
            relief(im, &hm, 135.0, k * 0.8, Some([200.0, 195.0, 185.0]), 0.55 - bright * 0.4);
        }
        "ScreenDoor" => {
            let bw = e.num("BackgroundColor", 0.0) < 1.0;
            let dens = n(e, "Density", 40.0);
            let soft = pct(e, "Softness", 85.0);
            let bright = pct(e, "Brightness", 100.0);
            let sp = (64.0 - dens).max(3.0) / 8.0 + 1.5;
            let b = gauss(im, soft * 2.0 * s);
            *im = build(&b, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let line = |v: f32| ((v / sp).fract() - 0.5).abs() * 2.0;
                let mesh = (1.0 - smooth(0.0, 0.25, line(ex).min(line(ey)).min(1.0) * 0.5 + 0.0)).max(0.0);
                let wire = if bw { gray3(if (x + y) % 2 == 0 { 30.0 } else { 40.0 }) } else { [p[0] * 0.35, p[1] * 0.35, p[2] * 0.35] };
                let c = [p[0] * bright, p[1] * bright, p[2] * bright];
                lerp3(c, wire, mesh * 0.8)
            });
        }
        "Stone" => {
            let rough = pct(e, "Roughness", 100.0);
            let detail = pct(e, "Detail", 100.0);
            let dir = from_dir(n(e, "LightDirection", 315.0));
            let inv = e.flag("Invert", false);
            let hm = hmap(im, cx, |ex, ey| {
                let v = fbm(ex * (0.02 + detail * 0.08), ey * (0.02 + detail * 0.08), 5, seed) * (2.0 + rough * 6.0);
                if inv {
                    -v
                } else {
                    v
                }
            });
            relief(im, &hm, dir, k, None, 0.0);
        }
        "Underpainting" => {
            let amt = n(e, "Amount", 10.0).max(1.0);
            let bright = 0.5 + pct(e, "Brightness", 50.0);
            let b = gauss(im, amt * 0.5 * s);
            *im = build(&b, |x, y, p| {
                let (ex, ey) = ep(cx, x, y);
                let (h, sat, l) = rgb_to_hsl(rgb(p));
                let j = (noise(ex * 0.3, ey * 0.3, seed) - 0.5) * 0.12;
                let o = hsl_to_rgb(h, sat * 0.8, (l + j).clamp(0.0, 1.0));
                [o[0] * bright, o[1] * bright, o[2] * bright]
            });
        }
        "BumpMap" => {
            let tile = e.num("ScalingMode", 0.0) >= 1.0;
            let (tw, th) = (n(e, "TileWidth", 2.0).max(1.0), n(e, "TileHeight", 2.0).max(1.0));
            let kind = ix(e, "CurrentBumpMapImage", 0);
            let invert = e.flag("Invert", false);
            let sc = n(e, "ScaleFactor", 100.0) / 100.0;
            let (floor, ceil) = (n(e, "Floor", 0.0) / 255.0, n(e, "Ceiling", 255.0) / 255.0);
            let hi = pct(e, "Highlight", 50.0);
            let lc = col(e.color("DirectionalLightColorItem", Color::WHITE));
            let dir = n(e, "Direction", 135.0);
            let decl = n(e, "Declination", 45.0).to_radians();
            let lb = pct(e, "DirectionalLightBrightness", 100.0);
            let ac = col(e.color("AmbientLightColorItem", Color::WHITE));
            let ab = pct(e, "AmbientLightBrightness", 50.0);
            let smooth_map = e.flag("Smooth", false);
            let (fw, fh) = (cx.fw() / s, cx.fh() / s);
            let mut hm = hmap(im, cx, |ex, ey| {
                let (u, v) = if tile { (ex / fw.max(1.0) * tw * 64.0, ey / fh.max(1.0) * th * 64.0) } else { (ex / fw.max(1.0) * 128.0, ey / fh.max(1.0) * 128.0) };
                let mut h = texture(kind, u, v, seed).clamp(floor, ceil.max(floor + 0.01));
                if invert {
                    h = 1.0 - h;
                }
                h * 6.0 * sc
            });
            if smooth_map {
                let tmp = Img { w: im.w, h: im.h, p: hm.iter().map(|v| [*v * 40.0, 0.0, 0.0, 255.0]).collect() };
                hm = gauss(&tmp, 2.0 * s).p.iter().map(|p| p[0] / 40.0).collect();
            }
            let sh = shade(&hm, im.w, im.h, dir, k * (1.0 - decl.sin() * 0.6));
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                let spec = ((sh[i] - 1.0).max(0.0) * 2.0).powi(2) * hi;
                for c in 0..3 {
                    let light = ac[c] / 255.0 * ab + lc[c] / 255.0 * lb * sh[i] * 0.7;
                    p[c] = clamp255(p[c] * light + 255.0 * spec);
                }
            }
        }
        _ => {}
    }
}
