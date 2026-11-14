//! CMYK as CorelDRAW 2026 shows it on screen, sampled from CorelDRAW itself for each CMYK profile
//! it comes with: the fallback of the color engine when the ICC profile file of a document's
//! CMYK profile is not installed.

/// Ink levels sampled per channel (every eighth of each ink).
pub const LEVELS: usize = 9;
/// Bytes of one grid: RGB for every sample. Index: ((c * 9 + m) * 9 + y) * 9 + k.
pub const GRID_LEN: usize = LEVELS * LEVELS * LEVELS * LEVELS * 3;

/// U.S. Web Coated (SWOP) v2 to sRGB, CorelDRAW's default.
pub const DEFAULT_GRID: &[u8; GRID_LEN] = include_bytes!("cmyk9.bin");

/// The same grid for each of the other CMYK profiles CorelDRAW 2026 comes with: the profile's
/// name (a length byte, then UTF-8), then its grid.
const PROFILE_GRIDS: &[u8] = include_bytes!("cmyk-profiles.bin");

/// The grid of a CMYK profile, by the name CorelDRAW gives it (`None` when it was not sampled).
pub fn profile_grid(name: &str) -> Option<&'static [u8]> {
    let name = name.trim();
    if name.eq_ignore_ascii_case(crate::document::DEFAULT_CMYK_PROFILE) {
        return Some(DEFAULT_GRID);
    }
    let mut at = 0;
    while at < PROFILE_GRIDS.len() {
        let n = PROFILE_GRIDS[at] as usize;
        let here = PROFILE_GRIDS.get(at + 1..at + 1 + n)?;
        let grid = PROFILE_GRIDS.get(at + 1 + n..at + 1 + n + GRID_LEN)?;
        if here.eq_ignore_ascii_case(name.as_bytes()) {
            return Some(grid);
        }
        at += 1 + n + GRID_LEN;
    }
    None
}

/// The names of the sampled profiles (besides the default one).
pub fn profile_names() -> Vec<String> {
    let mut v = vec![];
    let mut at = 0;
    while at < PROFILE_GRIDS.len() {
        let n = PROFILE_GRIDS[at] as usize;
        if let Some(here) = PROFILE_GRIDS.get(at + 1..at + 1 + n) {
            v.push(String::from_utf8_lossy(here).to_string());
        }
        at += 1 + n + GRID_LEN;
    }
    v
}

/// CMYK inks (each 0..1) to sRGB (each 0..255, not rounded), interpolating the grid.
pub fn sample(grid: &[u8], cmyk: [f64; 4]) -> [f64; 3] {
    if cmyk.iter().all(|v| *v <= 0.0) {
        return [255.0; 3];
    }
    let ink = cmyk.map(|v| v.clamp(0.0, 1.0) * (LEVELS - 1) as f64);
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
    rgb
}

/// sRGB (each 0..1) to CIE L*a*b* (D50, Bradford adapted), as ICC profiles connect.
pub fn srgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let lin = rgb.map(|v| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    let [r, g, b] = lin;
    // linear sRGB to XYZ D50 (Bradford)
    let x = 0.436_074_7 * r + 0.385_064_9 * g + 0.143_080_4 * b;
    let y = 0.222_504_5 * r + 0.716_878_6 * g + 0.060_616_9 * b;
    let z = 0.013_932_2 * r + 0.097_104_5 * g + 0.714_173_3 * b;
    let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
    let (fx, fy, fz) = (f(x / 0.964_22), f(y), f(z / 0.825_21));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIE L*a*b* (D50) to sRGB (each 0..1, clipped).
pub fn lab_to_srgb(lab: [f64; 3]) -> [f64; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    let inv = |t: f64| if t * t * t > 216.0 / 24389.0 { t * t * t } else { (116.0 * t - 16.0) * 27.0 / 24389.0 };
    let (x, y, z) = (inv(fx) * 0.964_22, inv(fy), inv(fz) * 0.825_21);
    let r = 3.133_856_1 * x - 1.616_866_7 * y - 0.490_614_6 * z;
    let g = -0.978_768_4 * x + 1.916_141_5 * y + 0.033_454_0 * z;
    let b = 0.071_945_3 * x - 0.228_991_4 * y + 1.405_242_7 * z;
    [r, g, b].map(|v| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    })
}

/// The black of a color with GCR: none for light colors, rising to 90% for black.
fn black_generation(rgb: [f64; 3]) -> f64 {
    let k = 1.0 - rgb[0].max(rgb[1]).max(rgb[2]);
    if k <= 0.3 {
        0.0
    } else {
        0.9 * ((k - 0.3) / 0.7).powf(1.6)
    }
}

/// sRGB (each 0..1) to CMYK inks (each 0..1) that print closest to it through `grid`: the black
/// from a GCR curve (more when the color cannot be reached without it), then the cyan, magenta
/// and yellow found by damped Gauss-Newton steps on the color difference.
pub fn invert(grid: &[u8], rgb: [f64; 3]) -> [f64; 4] {
    if rgb.iter().all(|v| *v >= 1.0) {
        return [0.0; 4];
    }
    let target = srgb_to_lab(rgb);
    let k0 = black_generation(rgb);
    let (mut best, mut best_d) = solve_cmy(grid, rgb, target, k0);
    // out of reach with that black: try more
    let mut k = k0;
    while best_d > 1.0 && k < 1.0 {
        k = (k + 0.05).min(1.0);
        let (c, d) = solve_cmy(grid, rgb, target, k);
        if d < best_d - 0.25 {
            best = c;
            best_d = d;
        } else if d > best_d + 2.0 {
            break;
        }
    }
    best
}

/// The cyan, magenta and yellow (with black `k`) closest to `target` (L*a*b*), and the ΔE.
fn solve_cmy(grid: &[u8], rgb: [f64; 3], target: [f64; 3], k: f64) -> ([f64; 4], f64) {
    let lab_of = |c: [f64; 3], k: f64| srgb_to_lab(sample(grid, [c[0], c[1], c[2], k]).map(|v| v / 255.0));
    let err = |lab: [f64; 3]| [lab[0] - target[0], lab[1] - target[1], lab[2] - target[2]];
    // start from the plain formula, adding the part of the black the K ink does not cover
    let kn = 1.0 - rgb[0].max(rgb[1]).max(rgb[2]);
    let mut c = if kn >= 0.999 { [0.0; 3] } else { [0, 1, 2].map(|i| ((1.0 - rgb[i] - kn) / (1.0 - kn)).clamp(0.0, 1.0)) };
    let extra = (kn - k).max(0.0);
    for v in c.iter_mut() {
        *v = (*v + extra * (1.0 - *v)).clamp(0.0, 1.0);
    }
    let mut lambda = 1e-3;
    let mut cur = err(lab_of(c, k));
    let mut cur_d = cur.iter().map(|v| v * v).sum::<f64>();
    for _ in 0..40 {
        if cur_d < 1e-4 {
            break;
        }
        // numeric Jacobian (3 x 3)
        let h = 1e-3;
        let mut jac = [[0.0f64; 3]; 3];
        for j in 0..3 {
            let mut p = c;
            let step = if p[j] + h > 1.0 { -h } else { h };
            p[j] += step;
            let e = err(lab_of(p, k));
            for i in 0..3 {
                jac[i][j] = (e[i] - cur[i]) / step;
            }
        }
        // (JᵀJ + λ diag) δ = -Jᵀ e
        let mut a = [[0.0f64; 3]; 3];
        let mut g = [0.0f64; 3];
        for r in 0..3 {
            for s in 0..3 {
                a[r][s] = (0..3).map(|i| jac[i][r] * jac[i][s]).sum();
            }
            g[r] = -(0..3).map(|i| jac[i][r] * cur[i]).sum::<f64>();
        }
        for (r, row) in a.iter_mut().enumerate() {
            row[r] *= 1.0 + lambda;
            row[r] += 1e-9;
        }
        let Some(d) = solve3(a, g) else { break };
        let next = [0, 1, 2].map(|i| (c[i] + d[i]).clamp(0.0, 1.0));
        let e = err(lab_of(next, k));
        let dn = e.iter().map(|v| v * v).sum::<f64>();
        if dn < cur_d {
            c = next;
            cur = e;
            cur_d = dn;
            lambda = (lambda * 0.3).max(1e-6);
        } else {
            lambda *= 10.0;
            if lambda > 1e6 {
                break;
            }
        }
    }
    ([c[0], c[1], c[2], k], cur_d.sqrt())
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = det(a);
    if d.abs() < 1e-18 {
        return None;
    }
    let mut x = [0.0; 3];
    for (i, xi) in x.iter_mut().enumerate() {
        let mut m = a;
        for r in 0..3 {
            m[r][i] = b[r];
        }
        *xi = det(m) / d;
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_by_name() {
        assert!(profile_grid("U.S. Web Coated (SWOP) v2").is_some());
        assert!(profile_grid("Coated FOGRA39 (ISO 12647-2:2004)").is_some());
        assert!(profile_grid("No Such Profile").is_none());
        assert!(profile_names().iter().any(|n| n == "Japan Color 2001 Coated"));
    }

    #[test]
    fn lab_round_trip() {
        for rgb in [[1.0, 0.0, 0.0], [0.2, 0.6, 0.9], [0.5, 0.5, 0.5]] {
            let back = lab_to_srgb(srgb_to_lab(rgb));
            for i in 0..3 {
                assert!((back[i] - rgb[i]).abs() < 1e-3, "{rgb:?} -> {back:?}");
            }
        }
        // sRGB red is L* 54.3 a* 80.8 b* 69.9 (D50)
        let l = srgb_to_lab([1.0, 0.0, 0.0]);
        assert!((l[0] - 54.29).abs() < 0.2 && (l[1] - 80.80).abs() < 0.5 && (l[2] - 69.89).abs() < 0.5, "{l:?}");
    }

    #[test]
    fn inverting_the_grid_finds_the_inks_back() {
        for ink in [[0.0, 1.0, 1.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.4, 0.1, 0.6, 0.0], [0.2, 0.3, 0.1, 0.0]] {
            let rgb = sample(DEFAULT_GRID, ink).map(|v| v / 255.0);
            let back = invert(DEFAULT_GRID, rgb);
            let again = sample(DEFAULT_GRID, back).map(|v| v / 255.0);
            let d = srgb_to_lab(rgb).iter().zip(srgb_to_lab(again)).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
            assert!(d < 1.0, "{ink:?}: {back:?} (delta E {d})");
        }
    }
}
