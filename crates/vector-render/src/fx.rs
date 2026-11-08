//! Pixel-level effects on premultiplied RGBA pixmaps.

use tiny_skia::Pixmap;

/// Approximate Gaussian blur with three box blur passes (on premultiplied data).
pub fn blur(p: &mut Pixmap, radius: f32) {
    if radius < 0.5 {
        return;
    }
    let w = p.width() as usize;
    let h = p.height() as usize;
    if w == 0 || h == 0 {
        return;
    }
    // box sizes for 3 passes approximating a gaussian with sigma = radius/2
    let sigma = (radius / 2.0).max(0.5);
    let boxes = boxes_for_gauss(sigma, 3);
    let data = p.data_mut();
    let mut buf: Vec<f32> = data.iter().map(|v| *v as f32).collect();
    let mut tmp = vec![0f32; buf.len()];
    for r in boxes {
        let r = ((r - 1) / 2) as usize;
        box_blur_h(&buf, &mut tmp, w, h, r);
        box_blur_v(&tmp, &mut buf, w, h, r);
    }
    for (d, s) in data.iter_mut().zip(buf.iter()) {
        *d = s.round().clamp(0.0, 255.0) as u8;
    }
    // keep premultiplied invariant: color <= alpha
    for px in data.chunks_exact_mut(4) {
        let a = px[3];
        px[0] = px[0].min(a);
        px[1] = px[1].min(a);
        px[2] = px[2].min(a);
    }
}

fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<i32> {
    let w_ideal = ((12.0 * sigma * sigma / n as f32) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - (n as f32) * (wl * wl) as f32 - 4.0 * n as f32 * wl as f32 - 3.0 * n as f32) / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round() as usize;
    (0..n).map(|i| if i < m { wl.max(1) } else { wu.max(1) }).collect()
}

fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let iarr = 1.0 / (r + r + 1) as f32;
    for y in 0..h {
        for c in 0..4 {
            let row = y * w * 4;
            let get = |x: isize| -> f32 {
                let xc = x.clamp(0, w as isize - 1) as usize;
                src[row + xc * 4 + c]
            };
            let mut acc = 0.0;
            for x in -(r as isize)..=(r as isize) {
                acc += get(x);
            }
            for x in 0..w {
                dst[row + x * 4 + c] = acc * iarr;
                acc += get(x as isize + r as isize + 1) - get(x as isize - r as isize);
            }
        }
    }
}

fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let iarr = 1.0 / (r + r + 1) as f32;
    for x in 0..w {
        for c in 0..4 {
            let get = |y: isize| -> f32 {
                let yc = y.clamp(0, h as isize - 1) as usize;
                src[yc * w * 4 + x * 4 + c]
            };
            let mut acc = 0.0;
            for y in -(r as isize)..=(r as isize) {
                acc += get(y);
            }
            for y in 0..h {
                dst[y * w * 4 + x * 4 + c] = acc * iarr;
                acc += get(y as isize + r as isize + 1) - get(y as isize - r as isize);
            }
        }
    }
}

/// Replace color with `rgb` keeping alpha (premultiplied), scaled by `opacity`.
pub fn tint(p: &mut Pixmap, rgb: [u8; 3], opacity: f32) {
    for px in p.data_mut().chunks_exact_mut(4) {
        let a = (px[3] as f32 * opacity).round().clamp(0.0, 255.0);
        let f = a / 255.0;
        px[0] = (rgb[0] as f32 * f).round() as u8;
        px[1] = (rgb[1] as f32 * f).round() as u8;
        px[2] = (rgb[2] as f32 * f).round() as u8;
        px[3] = a as u8;
    }
}

fn unpremul(px: &[u8]) -> (f32, f32, f32, f32) {
    let a = px[3] as f32;
    if a <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (px[0] as f32 * 255.0 / a, px[1] as f32 * 255.0 / a, px[2] as f32 * 255.0 / a, a)
}

fn premul(px: &mut [u8], r: f32, g: f32, b: f32, a: f32) {
    let f = a / 255.0;
    px[0] = (r.clamp(0.0, 255.0) * f).round() as u8;
    px[1] = (g.clamp(0.0, 255.0) * f).round() as u8;
    px[2] = (b.clamp(0.0, 255.0) * f).round() as u8;
    px[3] = a.clamp(0.0, 255.0).round() as u8;
}

pub fn map_colors(p: &mut Pixmap, mut f: impl FnMut(f32, f32, f32) -> (f32, f32, f32)) {
    for px in p.data_mut().chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        let (r, g, b, a) = unpremul(px);
        let (r2, g2, b2) = f(r, g, b);
        premul(px, r2, g2, b2, a);
    }
}

pub fn grayscale(p: &mut Pixmap) {
    map_colors(p, |r, g, b| {
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        (l, l, l)
    });
}

pub fn sepia(p: &mut Pixmap) {
    map_colors(p, |r, g, b| {
        (
            0.393 * r + 0.769 * g + 0.189 * b,
            0.349 * r + 0.686 * g + 0.168 * b,
            0.272 * r + 0.534 * g + 0.131 * b,
        )
    });
}

pub fn invert(p: &mut Pixmap) {
    map_colors(p, |r, g, b| (255.0 - r, 255.0 - g, 255.0 - b));
}

pub fn adjust(p: &mut Pixmap, brightness: f32, contrast: f32, saturation: f32) {
    let c = (contrast / 100.0 + 1.0).max(0.0);
    let s = (saturation / 100.0 + 1.0).max(0.0);
    let br = brightness * 2.55;
    map_colors(p, |r, g, b| {
        let mut r = (r - 128.0) * c + 128.0 + br;
        let mut g = (g - 128.0) * c + 128.0 + br;
        let mut b = (b - 128.0) * c + 128.0 + br;
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        r = l + (r - l) * s;
        g = l + (g - l) * s;
        b = l + (b - l) * s;
        (r, g, b)
    });
}

pub fn noise(p: &mut Pixmap, amount: f32, seed: u32) {
    let mut state = seed.wrapping_mul(2654435761).max(1);
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as f32 / u32::MAX as f32) - 0.5
    };
    let amt = amount * 2.55;
    map_colors(p, |r, g, b| {
        let n = rnd() * amt;
        (r + n, g + n, b + n)
    });
}

pub fn pixelate(p: &mut Pixmap, size: u32) {
    let size = size.max(1) as usize;
    let w = p.width() as usize;
    let h = p.height() as usize;
    let data = p.data_mut();
    let mut by = 0;
    while by < h {
        let mut bx = 0;
        while bx < w {
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for y in by..(by + size).min(h) {
                for x in bx..(bx + size).min(w) {
                    let i = (y * w + x) * 4;
                    for c in 0..4 {
                        acc[c] += data[i + c] as u32;
                    }
                    n += 1;
                }
            }
            if n > 0 {
                for y in by..(by + size).min(h) {
                    for x in bx..(bx + size).min(w) {
                        let i = (y * w + x) * 4;
                        for c in 0..4 {
                            data[i + c] = (acc[c] / n) as u8;
                        }
                    }
                }
            }
            bx += size;
        }
        by += size;
    }
}

pub fn emboss(p: &mut Pixmap, depth: f32) {
    let w = p.width() as usize;
    let h = p.height() as usize;
    let src = p.data().to_vec();
    let data = p.data_mut();
    let lum = |x: usize, y: usize| -> f32 {
        let i = (y * w + x) * 4;
        let (r, g, b, _) = unpremul(&src[i..i + 4]);
        0.299 * r + 0.587 * g + 0.114 * b
    };
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = (y * w + x) * 4;
            if src[i + 3] == 0 {
                continue;
            }
            let d = (lum(x + 1, y + 1) - lum(x - 1, y - 1)) * depth / 4.0;
            let v = 128.0 + d;
            premul(&mut data[i..i + 4], v, v, v, src[i + 3] as f32);
        }
    }
}

/// Simple bevel lighting: uses a blurred alpha as a height map.
pub fn bevel(p: &mut Pixmap, depth_px: f32, light_angle_deg: f32) {
    let w = p.width() as usize;
    let h = p.height() as usize;
    if w < 3 || h < 3 {
        return;
    }
    let mut height = Pixmap::new(w as u32, h as u32).unwrap();
    {
        let hd = height.data_mut();
        for (i, px) in p.data().chunks_exact(4).enumerate() {
            let a = px[3];
            hd[i * 4] = a;
            hd[i * 4 + 1] = a;
            hd[i * 4 + 2] = a;
            hd[i * 4 + 3] = a;
        }
    }
    blur(&mut height, depth_px.max(1.0));
    let hd = height.data().to_vec();
    let la = light_angle_deg.to_radians();
    let (lx, ly) = (la.cos(), -la.sin());
    let data = p.data_mut();
    let hv = |x: usize, y: usize| hd[(y * w + x) * 4 + 3] as f32;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = (y * w + x) * 4;
            if data[i + 3] == 0 {
                continue;
            }
            let gx = hv(x + 1, y) - hv(x - 1, y);
            let gy = hv(x, y + 1) - hv(x, y - 1);
            let shade = (gx * lx + gy * ly) / 255.0 * 2.0;
            let (r, g, b, a) = unpremul(&data[i..i + 4]);
            let (r, g, b) = if shade > 0.0 {
                (r + (255.0 - r) * shade.min(1.0), g + (255.0 - g) * shade.min(1.0), b + (255.0 - b) * shade.min(1.0))
            } else {
                let s = 1.0 + shade.max(-1.0);
                (r * s, g * s, b * s)
            };
            premul(&mut data[i..i + 4], r, g, b, a);
        }
    }
}

/// Composite `src` over `dst` with a blend mode implemented per pixel (for modes tiny-skia lacks we
/// fall back to its own blend modes; this helper multiplies alpha by `mask` values when provided).
pub fn multiply_alpha(p: &mut Pixmap, mask: &[u8]) {
    for (px, m) in p.data_mut().chunks_exact_mut(4).zip(mask.iter()) {
        let f = *m as f32 / 255.0;
        for c in px.iter_mut() {
            *c = (*c as f32 * f).round() as u8;
        }
    }
}
