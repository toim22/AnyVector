//! Texture fills of CorelDRAW 4 to X2.
//!
//! Those files keep no picture of a texture: they keep the name of its style and the style's
//! settings, and CorelDRAW draws the texture again from them. We do not have CorelDRAW's
//! generators, so we read the colors and the brightness from the settings and draw a soft cloud
//! tile in those colors. It has the texture's tones, not its exact drawing.

/// What we use of a texture fill: the number its drawing starts from and its colors, in the
/// order of the style's settings and with the brightness applied.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Texture {
    pub seed: u16,
    pub colors: Vec<[u8; 3]>,
}

/// The side of the tile in pixels, and in points.
const SIDE: usize = 128;
pub(super) const TILE: f64 = 144.0;

/// Read a texture fill record. The record has two names of 32 bytes each (the texture as the
/// user saved it, then the style that draws it), the number of settings, and the settings in
/// slots of 8 bytes: a number is the slot's first 16 bits, a color is three 16-bit percentages
/// of red, green and blue. Which slots are colors depends on the style, so an unknown style
/// gives `None`. The bytes after a number or after a name's end are leftovers and mean nothing.
pub(super) fn read(b: &[u8]) -> Option<Texture> {
    // The offset of the names changed between versions: look for the style's name instead,
    // followed 32 bytes later by a count that fits the style. The last one is the style.
    let mut found = None;
    for o in 8..b.len().saturating_sub(34) {
        if !b[o].is_ascii_alphabetic() {
            continue;
        }
        let Some(len) = b[o..o + 32].iter().position(|&c| c == 0) else { continue };
        let Ok(name) = std::str::from_utf8(&b[o..o + len]) else { continue };
        let Some(kinds) = kinds(name) else { continue };
        let count = u16::from_le_bytes([b[o + 32], b[o + 33]]) as usize;
        if count >= kinds.len() && count <= 24 && o + 34 + 8 * kinds.len() <= b.len() {
            found = Some((o + 34, kinds));
        }
    }
    let (at, kinds) = found?;
    let word = |i: usize, n: usize| i16::from_le_bytes([b[at + 8 * i + 2 * n], b[at + 8 * i + 2 * n + 1]]);
    let brightness = kinds.bytes().position(|k| k == b'b').map_or(0, |i| word(i, 0)).clamp(-100, 100) as f64 / 100.0;
    let channel = |v: i16| {
        let v = v.clamp(0, 100) as f64 / 100.0;
        let v = if brightness >= 0.0 { v + (1.0 - v) * brightness } else { v * (1.0 + brightness) };
        (v * 255.0).round() as u8
    };
    let colors = kinds.bytes().enumerate().filter(|(_, k)| *k == b'c').map(|(i, _)| [channel(word(i, 0)), channel(word(i, 1)), channel(word(i, 2))]).collect();
    Some(Texture { seed: word(0, 0) as u16, colors })
}

/// The kinds of a style's settings, in the file's order: `n` a number, `c` a color, `b` the
/// brightness. Styles that CorelDRAW 5 added are listed as "Cdr5:name", and its "-Lit" styles
/// were later renamed "Embossed".
fn kinds(name: &str) -> Option<&'static str> {
    let find = |n: &str| STYLES.iter().find(|(s, _)| *s == n).map(|(_, k)| *k);
    let renamed = name.replace("-Lit", " Embossed");
    find(name).or_else(|| find(&renamed)).or_else(|| find(&format!("Cdr5:{name}"))).or_else(|| find(&format!("Cdr5:{renamed}")))
}

/// Draw the tile: soft clouds that run through the texture's colors, and repeat without a seam.
/// Returns the pixels (RGBA) and the tile's width and height in pixels.
pub(super) fn tile(t: &Texture) -> Option<(Vec<u8>, u32, u32)> {
    let ramp: Vec<[f64; 3]> = match t.colors.as_slice() {
        [] => return None,
        // one color: from it to a lighter tone of it
        [c] => vec![c.map(f64::from), c.map(|v| (v as f64 + 255.0) / 2.0)],
        many => many.iter().map(|c| c.map(f64::from)).collect(),
    };
    // a number that looks random for each lattice point of each layer
    let hash = |x: usize, y: usize, layer: usize| {
        let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca6b) ^ (layer as u32).wrapping_mul(0xc2b2_ae35) ^ (t.seed as u32).wrapping_mul(0x27d4_eb2f);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        h = h.wrapping_mul(0x297a_2d39);
        h ^= h >> 15;
        (h & 0xffff) as f64 / 65535.0
    };
    // layers of smoothed noise, each twice as fine and half as strong as the one before
    let mut field = vec![0.0f64; SIDE * SIDE];
    for (layer, cells) in [4usize, 8, 16, 32].into_iter().enumerate() {
        let step = SIDE / cells;
        let weight = 0.5f64.powi(layer as i32);
        let ease = |v: usize| {
            let f = (v % step) as f64 / step as f64;
            f * f * (3.0 - 2.0 * f)
        };
        for y in 0..SIDE {
            for x in 0..SIDE {
                let (cx, cy) = (x / step, y / step);
                let (nx, ny) = ((cx + 1) % cells, (cy + 1) % cells);
                let (fx, fy) = (ease(x), ease(y));
                let top = hash(cx, cy, layer) * (1.0 - fx) + hash(nx, cy, layer) * fx;
                let bottom = hash(cx, ny, layer) * (1.0 - fx) + hash(nx, ny, layer) * fx;
                field[y * SIDE + x] += weight * (top * (1.0 - fy) + bottom * fy);
            }
        }
    }
    let (low, high) = field.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let span = (high - low).max(1e-9);
    let mut rgba = Vec::with_capacity(SIDE * SIDE * 4);
    for v in field {
        let at = (v - low) / span * (ramp.len() - 1) as f64;
        let i = (at.floor() as usize).min(ramp.len() - 2);
        let f = at - i as f64;
        for c in 0..3 {
            rgba.push((ramp[i][c] + (ramp[i + 1][c] - ramp[i][c]) * f).round().clamp(0.0, 255.0) as u8);
        }
        rgba.push(255);
    }
    Some((rgba, SIDE as u32, SIDE as u32))
}

/// The styles of CorelDRAW's texture library with the kinds of their settings (see `kinds`).
const STYLES: &[(&str, &str)] = &[
    ("Aerial Photography", "nnccb"),
    ("Blend Corners", "ccccb"),
    ("Blend Edges", "ccccb"),
    ("Cosmic Clouds", "nnnnnb"),
    ("Cosmic Energy", "nccccccb"),
    ("Cosmic Minerals", "nnnnnnnb"),
    ("Drapes", "nnccb"),
    ("Fiber", "nnnncccb"),
    ("Flames", "nnccb"),
    ("Mineral.Cloudy 2 Colors", "nnnnnccb"),
    ("Mineral.Cloudy 3 Colors", "nnnnncccb"),
    ("Mineral.Cloudy 5 Colors", "nnnnncccccb"),
    ("Mineral.Fractal 2 Colors", "nnnnnccb"),
    ("Mineral.Fractal 3 Colors", "nnnnncccb"),
    ("Mineral.Fractal 5 Colors", "nnnnncccccb"),
    ("Mineral.Speckled 2 Colors", "nnnnnccb"),
    ("Mineral.Speckled 3 Colors", "nnnnncccb"),
    ("Mineral.Speckled 5 Colors", "nnnnncccccb"),
    ("Mineral.Swirled 2 Colors", "nnnnnccb"),
    ("Mineral.Swirled 3 Colors", "nnnnncccb"),
    ("Mineral.Swirled 5 Colors", "nnnnncccccb"),
    ("Noise", "ncb"),
    ("Noise.Blended", "nccb"),
    ("Noise.Rainbow Blended", "nccb"),
    ("Recycled Paper", "nnnncccb"),
    ("Recycled Paper Rainbow", "nnnncccb"),
    ("Satellite Photography", "nnnnbncccccc"),
    ("Scribbles", "nnnnccb"),
    ("Sky 2 Colors", "nnnccb"),
    ("Sky 3 Colors", "nnncccb"),
    ("Sky 5 Colors", "nnncccccb"),
    ("Smooth Weave", "nnccb"),
    ("Surfaces", "nnnnnnccb"),
    ("Swirls", "nnccb"),
    ("Swirls2", "nnccb"),
    ("Texture Blend Horizontal", "nccb"),
    ("Texture Blend Vertical", "nccb"),
    ("Threads", "nnnncccb"),
    ("Threads Rainbow", "nnnncccb"),
    ("Water 2 Colors", "nnnnccb"),
    ("Water 3 Colors", "nnnncccb"),
    ("Water 5 Colors", "nnnncccccb"),
    ("Water Color", "nccccb"),
    ("Cdr5:Biology1 2C", "nnnccb"),
    ("Cdr5:Biology1 3C", "nnncccb"),
    ("Cdr5:Biology1 5C", "nnncccccb"),
    ("Cdr5:Biology2 2C", "nnnccb"),
    ("Cdr5:Biology2 3C", "nnncccb"),
    ("Cdr5:Biology2 5C", "nnncccccb"),
    ("Cdr5:BubbleMania Hard 2C", "nnnnnnnnccb"),
    ("Cdr5:BubbleMania Hard 3C", "nnnnnnnncccb"),
    ("Cdr5:BubbleMania Soft 2C", "nnnnnnnnccb"),
    ("Cdr5:BubbleMania Soft 3C", "nnnnnnnncccb"),
    ("Cdr5:Cotton 2C", "nnnccb"),
    ("Cdr5:Cotton 3C", "nnncccb"),
    ("Cdr5:Cotton 5C", "nnncccccb"),
    ("Cdr5:Eclipse 2 Colors", "nnnnnnccb"),
    ("Cdr5:Fiber Embossed", "nnnnnnnnccb"),
    ("Cdr5:Hypnotic1 2 Colors", "nnnnnnccb"),
    ("Cdr5:Hypnotic2 2 Colors", "nnnnccb"),
    ("Cdr5:Leather 2C", "nnnnnnccb"),
    ("Cdr5:Leather 3C", "nnnnnncccb"),
    ("Cdr5:Leather 5C", "nnnnnncccccb"),
    ("Cdr5:Mandel0", "nnnnnnccb"),
    ("Cdr5:Patches 2C", "nnnnccb"),
    ("Cdr5:Patches 3C", "nnnncccb"),
    ("Cdr5:Patches 5C", "nnnncccccb"),
    ("Cdr5:Pottery Wheel 2 Colors", "nnnnnnnnnccb"),
    ("Cdr5:Putty 2C", "nnnnnnnnccb"),
    ("Cdr5:Putty 3C", "nnnnnnnncccb"),
    ("Cdr5:Putty 5C", "nnnnnncccccb"),
    ("Cdr5:Rain Drops Hard 2C", "nnnnnnnnccb"),
    ("Cdr5:Rain Drops Soft 2C", "nnnnnnnnccb"),
    ("Cdr5:Rain Drops Hard 3C", "nnnnnnnncccb"),
    ("Cdr5:Rain Drops Soft 3C", "nnnnnnnncccb"),
    ("Cdr5:Rings Hard 2C", "nnnnnnccb"),
    ("Cdr5:Rings Soft 2C", "nnnnnnccb"),
    ("Cdr5:Rings Hard 3C", "nnnnnncccb"),
    ("Cdr5:Rings Soft 3C", "nnnnnncccb"),
    ("Cdr5:Ripples Hard Embossed 2C", "nnnnnnnnnccb"),
    ("Cdr5:Ripples Soft Embossed 2C", "nnnnnnnccb"),
    ("Cdr5:Rock-Cloudy-Cracked 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Cloudy-Eroded 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Fractal-Cracked 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Fractal-Eroded 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Speckled-Cracked 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Speckled-Eroded 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Swirled-Cracked 2C", "nnnnnccnnncb"),
    ("Cdr5:Rock-Swirled-Eroded 2C", "nnnnnccnnncb"),
    ("Cdr5:Scribbles Embossed", "nnnnnnnnccb"),
    ("Cdr5:Spots Hard 2C", "nnnnnnccb"),
    ("Cdr5:Spots Hard 3C", "nnnnnncccb"),
    ("Cdr5:Spots Soft 2C", "nnnnnnccb"),
    ("Cdr5:Spots Soft 3C", "nnnnnncccb"),
    ("Cdr5:Surfaces 3C", "nnnnnncccb"),
    ("Cdr5:Surfaces 5C", "nnnnnncccccb"),
    ("Cdr5:Surfaces-Rainbow", "nnnnnnnnccb"),
    ("Cdr5:Surfaces1 2C", "nnnnnnnccb"),
    ("Cdr5:Surfaces1 3C", "nnnnnnncccb"),
    ("Cdr5:Surfaces1 5C", "nnnnnncccccb"),
    ("Cdr5:Swirls2 Horizontal", "nnccb"),
    ("Cdr5:Threads Embossed", "nnnnnnnnccb"),
    ("Cdr5:Vapor 2C", "nnnccb"),
    ("Cdr5:Vapor 3C", "nnncccb"),
    ("Cdr5:Vapor 5C", "nnncccccb"),
    ("Cdr5:Water Color2 2C", "nnnccb"),
    ("Cdr5:Water Color2 3C", "nnncccb"),
    ("Cdr5:Water Color2 5C", "nnncccccb"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A record as CorelDRAW 6 writes it: names at 92 and 124, the settings from 156.
    fn record(saved: &str, style: &str, slots: &[[i16; 4]]) -> Vec<u8> {
        let mut b = vec![0u8; 158 + 8 * slots.len() + 16];
        b[4] = 11;
        b[92..92 + saved.len()].copy_from_slice(saved.as_bytes());
        b[124..124 + style.len()].copy_from_slice(style.as_bytes());
        b[156] = slots.len() as u8;
        for (i, s) in slots.iter().enumerate() {
            for (n, v) in s.iter().enumerate() {
                b[158 + 8 * i + 2 * n..][..2].copy_from_slice(&v.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn texture_colors_come_from_the_styles_settings() {
        // "Sky 2 Colors": number, softness, density, sky, clouds, brightness. The values after
        // the density and the brightness are leftovers, as CorelDRAW leaves them.
        let b = record("My sky", "Sky 2 Colors", &[[29335, 0, 0, 0], [0, 0, 0, 0], [50, 7, 7, 0], [0, 44, 100, 0], [100, 100, 100, 0], [0, 99, 85, 0]]);
        let t = read(&b).expect("a known style");
        assert_eq!(t.seed, 29335);
        assert_eq!(t.colors, vec![[0, 112, 255], [255, 255, 255]]);
    }

    #[test]
    fn texture_brightness_darkens_or_lightens_the_colors() {
        let slots = |brightness| [[1, 0, 0, 0], [0, 0, 0, 0], [50, 0, 0, 0], [0, 40, 100, 0], [100, 100, 100, 0], [brightness, 0, 0, 0]];
        let dark = read(&record("", "Sky 2 Colors", &slots(-50))).unwrap();
        assert_eq!(dark.colors, vec![[0, 51, 128], [128, 128, 128]]);
        let light = read(&record("", "Sky 2 Colors", &slots(50))).unwrap();
        assert_eq!(light.colors, vec![[128, 179, 255], [255, 255, 255]]);
    }

    #[test]
    fn texture_styles_of_coreldraw_5_are_found_by_their_old_names() {
        // "Threads-Lit" became "Cdr5:Threads Embossed": eight numbers, two colors, brightness
        let mut slots = vec![[0i16, 0, 0, 0]; 11];
        slots[8] = [80, 40, 20, 0];
        slots[9] = [100, 100, 100, 0];
        let t = read(&record("Threads-Lit", "Cdr5:Threads-Lit", &slots)).unwrap();
        assert_eq!(t.colors, vec![[204, 102, 51], [255, 255, 255]]);
        assert!(read(&record("x", "No such style", &slots)).is_none());
    }

    #[test]
    fn texture_tile_uses_its_colors_and_repeats() {
        let t = Texture { seed: 7, colors: vec![[0, 0, 255], [255, 255, 255]] };
        let (rgba, w, h) = tile(&t).unwrap();
        assert_eq!((w, h), (128, 128));
        assert_eq!(rgba.len(), 128 * 128 * 4);
        // red and green rise together from the blue to the white, blue stays full
        assert!(rgba.chunks(4).all(|p| p[0] == p[1] && p[2] == 255 && p[3] == 255));
        assert!(rgba.chunks(4).any(|p| p[0] == 0) && rgba.chunks(4).any(|p| p[0] == 255));
        // no seam: the last column is close to the first one
        let jump = (0..128).map(|y| (rgba[(y * 128 + 127) * 4] as i32 - rgba[y * 128 * 4] as i32).abs()).max().unwrap();
        assert!(jump < 40, "{jump}");
        assert!(tile(&Texture { seed: 7, colors: vec![] }).is_none());
    }
}
