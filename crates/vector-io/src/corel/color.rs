//! Colors of a CorelDRAW drawing: the stored color models, converted to display RGB.
use super::palettes::Kind;

use super::riff::Rd;
use vector_core::color::Color;

/// How CMYK looks on screen with CorelDRAW's default color management (U.S. Web Coated SWOP to
/// sRGB), as CorelDRAW 2026 converts it: RGB bytes at every eighth of each ink, 9 levels each.
/// Index: ((c * 9 + m) * 9 + y) * 9 + k.
const CMYK_GRID: &[u8; 9 * 9 * 9 * 9 * 3] = include_bytes!("cmyk9.bin");
const LEVELS: usize = 9;
const GRID_LEN: usize = 9 * 9 * 9 * 9 * 3;

/// The same grid for each of the other CMYK profiles CorelDRAW 2026 comes with: the profile's
/// name (a length byte, then UTF-8), then its grid.
const PROFILE_GRIDS: &[u8] = include_bytes!("cmyk-profiles.bin");

thread_local! {
    /// The grid of the drawing being read.
    static GRID: std::cell::Cell<&'static [u8]> = const { std::cell::Cell::new(CMYK_GRID) };
}

/// The grid of a CMYK profile, by the name CorelDRAW gives it.
fn profile_grid(name: &str) -> Option<&'static [u8]> {
    let mut at = 0;
    while at < PROFILE_GRIDS.len() {
        let n = PROFILE_GRIDS[at] as usize;
        let here = PROFILE_GRIDS.get(at + 1..at + 1 + n)?;
        let grid = PROFILE_GRIDS.get(at + 1 + n..at + 1 + n + GRID_LEN)?;
        if here.eq_ignore_ascii_case(name.trim().as_bytes()) {
            return Some(grid);
        }
        at += 1 + n + GRID_LEN;
    }
    None
}

/// Run `f` with CMYK shown through the named profile (the drawing's own), or the default one
/// when it is not known.
pub fn with_profile<T>(name: Option<&str>, f: impl FnOnce() -> T) -> T {
    let grid = name.and_then(profile_grid).unwrap_or(CMYK_GRID);
    let before = GRID.with(|g| g.replace(grid));
    let out = f();
    GRID.with(|g| g.set(before));
    out
}

/// CMYK (each 0..1) to display RGB, interpolating the sampled grid.
pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Color {
    let grid = GRID.with(|g| g.get());
    // pure white paper stays white
    if c <= 0.0 && m <= 0.0 && y <= 0.0 && k <= 0.0 {
        return Color::WHITE;
    }
    let ink = [c, m, y, k].map(|v| v.clamp(0.0, 1.0) * (LEVELS - 1) as f64);
    let lo = ink.map(|v| (v.floor() as usize).min(LEVELS - 2));
    let t = [0, 1, 2, 3].map(|i| ink[i] - lo[i] as f64);
    let mut rgb = [0.0f64; 3];
    for corner in 0..16usize {
        let mut w = 1.0;
        let mut idx = 0usize;
        for i in 0..4 {
            let hi = (corner >> i) & 1;
            w *= if hi == 1 { t[i] } else { 1.0 - t[i] };
            idx = idx * LEVELS + lo[i] + hi;
        }
        if w > 0.0 {
            for (ch, v) in rgb.iter_mut().enumerate() {
                *v += w * grid[idx * 3 + ch] as f64;
            }
        }
    }
    Color::rgb(rgb[0].round() as u8, rgb[1].round() as u8, rgb[2].round() as u8)
}

fn hsb(h: f64, s: f64, v: f64) -> Color {
    Color::from_hsb(h.rem_euclid(360.0), (s * 100.0).clamp(0.0, 100.0), (v * 100.0).clamp(0.0, 100.0))
}

fn hls(h: f64, l: f64, s: f64) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::rgb(f(r), f(g), f(b))
}

fn lab(l: f64, a: f64, b: f64) -> Color {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let inv = |t: f64| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    // D50 white, Bradford-adapted XYZ to linear sRGB
    let (x, y, z) = (0.9642 * inv(fx), inv(fy), 0.8249 * inv(fz));
    let r = 3.1338561 * x - 1.6168667 * y - 0.4906146 * z;
    let g = -0.9787684 * x + 1.9161415 * y + 0.0334540 * z;
    let bl = 0.0719453 * x - 0.2289914 * y + 1.4052427 * z;
    let gamma = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round().clamp(0.0, 255.0) as u8
    };
    Color::rgb(gamma(r), gamma(g), gamma(bl))
}

/// A color value in one of CorelDRAW's color models.
pub fn convert(model: u16, v: u32) -> Color {
    let b = v.to_le_bytes();
    let f = |i: usize| b[i] as f64;
    match model {
        // CMYK, inks 0..100
        0x01 | 0x02 | 0x15 => cmyk(f(0) / 100.0, f(1) / 100.0, f(2) / 100.0, f(3) / 100.0),
        // CMYK, inks 0..255
        0x03 | 0x11 => cmyk(f(0) / 255.0, f(1) / 255.0, f(2) / 255.0, f(3) / 255.0),
        0x04 => cmyk(f(0) / 255.0, f(1) / 255.0, f(2) / 255.0, 0.0),
        0x05 => Color::rgb(b[2], b[1], b[0]),
        0x06 => hsb(u16::from_le_bytes([b[0], b[1]]) as f64, f(2) / 255.0, f(3) / 255.0),
        0x07 => hls(u16::from_le_bytes([b[0], b[1]]) as f64, f(2) / 255.0, f(3) / 255.0),
        0x08 => {
            if b[0] != 0 {
                Color::WHITE
            } else {
                Color::BLACK
            }
        }
        0x09 => Color::rgb(b[0], b[0], b[0]),
        0x0b => {
            // YIQ, 0..255 with I and Q centered on 128
            let (y, i, q) = (f(0) / 255.0, (f(1) - 128.0) / 255.0 * 1.19, (f(2) - 128.0) / 255.0 * 1.05);
            let c = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
            Color::rgb(c(y + 0.956 * i + 0.621 * q), c(y - 0.272 * i - 0.647 * q), c(y - 1.106 * i + 1.703 * q))
        }
        0x0c => lab(f(0) * 100.0 / 255.0, b[1] as i8 as f64, b[2] as i8 as f64),
        0x12 => lab(f(0) * 100.0 / 255.0, f(1) - 128.0, f(2) - 128.0),
        // registration color: prints on every plate
        0x14 => Color::BLACK,
        _ => Color::BLACK,
    }
}

/// A color of one of CorelDRAW's fixed palettes (Pantone and the like): the drawing stores the
/// palette, the color's index (low word) and its tint in percent (high word).
fn fixed_palette(model: u16, palette: u16, v: u32) -> Color {
    let (ix, tint) = ((v & 0xffff) as usize, ((v >> 16) as u32).min(100));
    let row = super::palettes::table(model, palette).and_then(|(kind, from_zero, rows)| {
        let i = if from_zero { Some(ix) } else { ix.checked_sub(1) };
        i.and_then(|i| rows.get(i)).map(|row| (kind, *row))
    });
    let Some((kind, [a, b, c, d])) = row else {
        // a palette not known: show the tint as gray
        let g = (255.0 - tint as f64 * 1.28).round().clamp(0.0, 255.0) as u8;
        return Color::rgb(g, g, g);
    };
    let pack = |x: [u32; 4]| x[0] | x[1] << 8 | x[2] << 16 | x[3] << 24;
    match kind {
        // the inks scaled by the tint
        Kind::Cmyk => convert(0x02, pack([a, b, c, d].map(|x| x as u32 * tint / 100))),
        // the color mixed with white by the tint
        Kind::Rgb => {
            let mix = |x: u8| ((x as u32 * tint + 255 * (100 - tint)) / 100) as u8;
            Color::rgb(mix(a), mix(b), mix(c))
        }
        Kind::Lab => convert(0x12, pack([a as u32, b as u32, c as u32, 0])),
    }
}

/// Read a color record. Its layout depends on the file version.
pub fn read(r: &mut Rd, ver: u32) -> Color {
    if ver >= 500 {
        let mut model = r.u16();
        if model == 0x01 && ver >= 1300 {
            model = 0x19;
        }
        // CorelDRAW X7 can write a color of palette 0x1e with that palette as its model
        if model == 0x1e {
            let v = r.u32();
            r.skip(6);
            return fixed_palette(0x19, 0x1e, v);
        }
        let palette = r.u16();
        r.skip(4);
        let v = r.u32();
        if model == 0x19 || model == 0x0e {
            return fixed_palette(model, palette, v);
        }
        convert(model, v)
    } else if ver >= 400 {
        let model = r.u16();
        let c = r.u16() as u32 & 0xff;
        let m = r.u16() as u32 & 0xff;
        let y = r.u16() as u32 & 0xff;
        let k = r.u16() as u32 & 0xff;
        r.skip(2);
        convert(model, c | (m << 8) | (y << 16) | (k << 24))
    } else {
        let model = r.u8() as u16;
        let v = r.u32();
        convert(model, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_palette_colors_come_from_the_tables() {
        // palette 0x23, first color, full and half tint
        let row = super::super::palettes::table(0x19, 0x23).unwrap().2[0];
        let full = fixed_palette(0x19, 0x23, 1 | 100 << 16);
        assert_eq!(full, convert(0x02, u32::from_le_bytes(row)));
        let half = fixed_palette(0x19, 0x23, 1 | 50 << 16);
        assert!(half.r >= full.r && half.g >= full.g && half.b >= full.b && half != full);
        // palettes numbered from 0, and RGB palettes mixed with white
        assert_ne!(fixed_palette(0x19, 0x1a, 0 | 100 << 16), Color::rgb(128, 128, 128));
        assert_eq!(fixed_palette(0x19, 0x1e, 1), Color::WHITE);
        // an unknown palette keeps the gray of its tint
        assert_eq!(fixed_palette(0x19, 0x7f, 1 | 100 << 16), Color::rgb(127, 127, 127));
    }

    #[test]
    fn cmyk_matches_the_sampled_grid() {
        assert_eq!(cmyk(0.0, 0.0, 0.0, 0.0), Color::WHITE);
        assert_eq!(cmyk(0.0, 1.0, 1.0, 0.0), Color::rgb(237, 50, 55));
        assert_eq!(cmyk(1.0, 0.0, 0.0, 0.0), Color::rgb(0, 175, 239));
        assert_eq!(cmyk(0.0, 0.0, 0.0, 1.0), Color::rgb(55, 52, 53));
        assert_eq!(cmyk(0.5, 0.5, 0.5, 0.5), Color::rgb(91, 84, 82));
    }

    #[test]
    fn cmyk_between_the_samples_is_close_to_coreldraw() {
        // inks in percent, and the RGB CorelDRAW 2026 shows for them
        let seen: [([u8; 4], [u8; 3]); 12] = [
            ([41, 19, 50, 83], [60, 69, 59]),
            ([6, 9, 68, 12], [216, 198, 108]),
            ([46, 74, 7, 64], [82, 56, 85]),
            ([27, 4, 11, 55], [106, 126, 129]),
            ([53, 8, 30, 11], [114, 173, 169]),
            ([70, 54, 7, 72], [52, 59, 84]),
            ([15, 28, 80, 80], [83, 72, 44]),
            ([74, 7, 73, 74], [37, 80, 59]),
            ([50, 6, 28, 5], [125, 186, 182]),
            ([71, 17, 37, 53], [51, 102, 102]),
            ([18, 69, 15, 73], [93, 56, 73]),
            ([39, 71, 87, 23], [140, 89, 65]),        ];
        for (ink, rgb) in seen {
            let [c, m, y, k] = ink.map(|v| v as f64 / 100.0);
            let got = cmyk(c, m, y, k);
            let off = [got.r.abs_diff(rgb[0]), got.g.abs_diff(rgb[1]), got.b.abs_diff(rgb[2])];
            assert!(off.iter().all(|d| *d <= 6), "{ink:?}: {got:?} against {rgb:?}");
        }
    }

    #[test]
    fn models() {
        // RGB is stored blue first
        assert_eq!(convert(5, u32::from_le_bytes([220, 80, 32, 0])), Color::rgb(32, 80, 220));
        assert_eq!(convert(2, u32::from_le_bytes([0, 100, 100, 0])), Color::rgb(237, 50, 55));
        assert_eq!(convert(9, 128), Color::rgb(128, 128, 128));
        let mut r = Rd::new(&[5, 0, 5, 0, 0, 0, 0, 0, 0, 0, 200, 0]);
        assert_eq!(read(&mut r, 1500), Color::rgb(200, 0, 0));
    }
}
