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

/// A texture fill record as read: what to draw, and what the record says about its style.
#[derive(Clone, Debug)]
pub(super) struct Found {
    pub texture: Texture,
    /// The style's name as the record spells it.
    pub style: String,
    /// The style is in `STYLES`. When it isn't, `texture` is a neutral grey tile from the
    /// record's first setting, so that two unknown textures still look different.
    pub known: bool,
    /// The names are UTF-16 in 64-byte fields (CorelDRAW 12), not one byte per character.
    pub wide: bool,
}

/// A place in the record where a style's name could start, with the setting count after it.
#[derive(Clone)]
struct Candidate {
    /// The first byte of the settings.
    at: usize,
    name: String,
    count: usize,
    wide: bool,
}

/// Read a texture fill record. The record has names (the texture as the user saved it, then the
/// style that draws it), the number of settings, and the settings in slots of 8 bytes: a number
/// is the slot's first 16 bits, a color is three 16-bit percentages of red, green and blue.
/// Which slots are colors depends on the style. The bytes after a number or after a name's end
/// are leftovers and mean nothing.
///
/// CorelDRAW 4 to 6 write the names one byte per character in 32-byte fields. CorelDRAW 12
/// writes three names in UTF-16 in 64-byte fields (the texture, its library entry, the style),
/// so the count comes 64 bytes after the style's name.
///
/// A style without color settings (Cosmic Clouds, Cosmic Minerals) is drawn in greys from black
/// to white. A style that isn't in `STYLES` keeps its name and gets a neutral grey tile. `None`
/// when no name in the record can be the style's.
pub(super) fn read(b: &[u8]) -> Option<Found> {
    // The offset of the names changed between versions: look for the style's name instead,
    // followed one field later by a count that fits the style. The last one is the style, and
    // a count that is exactly the style's number of settings wins over one that is larger.
    let mut known: Option<(Candidate, &'static str)> = None;
    let mut exact: Option<(Candidate, &'static str)> = None;
    let mut unknown: Option<Candidate> = None;
    for o in 8..b.len().saturating_sub(34) {
        if !b[o].is_ascii_alphabetic() {
            continue;
        }
        let Some(c) = (if b[o + 1] == 0 { wide_name(b, o) } else { narrow_name(b, o) }) else { continue };
        match kinds(&c.name) {
            Some(kinds) => {
                // the settings the style reads must be in the record
                if c.count < kinds.len() || c.count > 24 || c.at + 8 * kinds.len() > b.len() {
                    continue;
                }
                if c.count == kinds.len() {
                    exact = Some((c.clone(), kinds));
                }
                known = Some((c, kinds));
            }
            None => {
                // a name of its own, not the tail of another one, and all its settings in the record
                let starts = if c.wide { !(0x20..0x7f).contains(&b[o - 2]) || b[o - 1] != 0 } else { !(0x20..0x7f).contains(&b[o - 1]) };
                let printable = c.name.chars().count() >= 2 && c.name.chars().all(|ch| !ch.is_control());
                if starts && printable && (1..=24).contains(&c.count) && c.at + 8 * c.count <= b.len() {
                    unknown = Some(c);
                }
            }
        }
    }
    let word = |at: usize, i: usize, n: usize| i16::from_le_bytes([b[at + 8 * i + 2 * n], b[at + 8 * i + 2 * n + 1]]);
    let Some((c, kinds)) = exact.or(known) else {
        let c = unknown?;
        let texture = Texture { seed: word(c.at, 0, 0) as u16, colors: vec![[80; 3], [208; 3]] };
        return Some(Found { texture, style: c.name, known: false, wide: c.wide });
    };
    let at = c.at;
    let brightness = kinds.bytes().position(|k| k == b'b').map_or(0, |i| word(at, i, 0)).clamp(-100, 100) as f64 / 100.0;
    let channel = |v: i16| {
        let v = v.clamp(0, 100) as f64 / 100.0;
        let v = if brightness >= 0.0 { v + (1.0 - v) * brightness } else { v * (1.0 + brightness) };
        (v * 255.0).round() as u8
    };
    let mut colors: Vec<[u8; 3]> = kinds.bytes().enumerate().filter(|(_, k)| *k == b'c').map(|(i, _)| [channel(word(at, i, 0)), channel(word(at, i, 1)), channel(word(at, i, 2))]).collect();
    if colors.is_empty() {
        // a style with no colors draws in greys: black to white, with the brightness applied
        colors = vec![[channel(0); 3], [channel(100); 3]];
    }
    Some(Found { texture: Texture { seed: word(at, 0, 0) as u16, colors }, style: c.name, known: true, wide: c.wide })
}

/// A one-byte name at `o` in a 32-byte field, then the count.
fn narrow_name(b: &[u8], o: usize) -> Option<Candidate> {
    let len = b[o..o + 32].iter().position(|&c| c == 0)?;
    let name = std::str::from_utf8(&b[o..o + len]).ok()?.to_string();
    let count = u16::from_le_bytes([b[o + 32], b[o + 33]]) as usize;
    Some(Candidate { at: o + 34, name, count, wide: false })
}

/// A UTF-16LE name at `o` in a 64-byte field, then the count.
fn wide_name(b: &[u8], o: usize) -> Option<Candidate> {
    if o + 66 > b.len() {
        return None;
    }
    let units: Vec<u16> = b[o..o + 64].chunks(2).map(|u| u16::from_le_bytes([u[0], u[1]])).collect();
    let len = units.iter().position(|&u| u == 0)?;
    let name = char::decode_utf16(units[..len].iter().copied()).collect::<Result<String, _>>().ok()?;
    let count = u16::from_le_bytes([b[o + 64], b[o + 65]]) as usize;
    Some(Candidate { at: o + 66, name, count, wide: true })
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
        let t = read(&b).expect("a known style").texture;
        assert_eq!(t.seed, 29335);
        assert_eq!(t.colors, vec![[0, 112, 255], [255, 255, 255]]);
    }

    #[test]
    fn texture_brightness_darkens_or_lightens_the_colors() {
        let slots = |brightness| [[1, 0, 0, 0], [0, 0, 0, 0], [50, 0, 0, 0], [0, 40, 100, 0], [100, 100, 100, 0], [brightness, 0, 0, 0]];
        let dark = read(&record("", "Sky 2 Colors", &slots(-50))).unwrap().texture;
        assert_eq!(dark.colors, vec![[0, 51, 128], [128, 128, 128]]);
        let light = read(&record("", "Sky 2 Colors", &slots(50))).unwrap().texture;
        assert_eq!(light.colors, vec![[128, 179, 255], [255, 255, 255]]);
    }

    #[test]
    fn texture_styles_of_coreldraw_5_are_found_by_their_old_names() {
        // "Threads-Lit" became "Cdr5:Threads Embossed": eight numbers, two colors, brightness
        let mut slots = vec![[0i16, 0, 0, 0]; 11];
        slots[8] = [80, 40, 20, 0];
        slots[9] = [100, 100, 100, 0];
        let t = read(&record("Threads-Lit", "Cdr5:Threads-Lit", &slots)).unwrap().texture;
        assert_eq!(t.colors, vec![[204, 102, 51], [255, 255, 255]]);
        // a style that isn't in the library is kept, by its name
        let other = read(&record("x", "No such style", &slots)).unwrap();
        assert!(!other.known);
        assert_eq!(other.style, "No such style");
    }

    /// A record as CorelDRAW 12 writes it: three UTF-16 names in 64-byte fields from 0x3c (the
    /// texture, its library entry, the style), the count at 0xfc and the settings from 0xfe.
    fn wide_record(style: &str, slots: &[[i16; 4]]) -> Vec<u8> {
        let mut b = vec![0u8; 0xfe + 8 * slots.len() + 30];
        b[4] = 11;
        b[0x0c..0x10].copy_from_slice(&508_000u32.to_le_bytes());
        b[0x10..0x14].copy_from_slice(&508_000u32.to_le_bytes());
        for (at, name) in [(0x3c, "CorelDefaultID_10001"), (0x7c, "CorelDefaultID_10334"), (0xbc, style)] {
            for (i, u) in name.encode_utf16().enumerate() {
                b[at + 2 * i..][..2].copy_from_slice(&u.to_le_bytes());
            }
        }
        b[0xfc..0xfe].copy_from_slice(&(slots.len() as u16).to_le_bytes());
        for (i, s) in slots.iter().enumerate() {
            for (n, v) in s.iter().enumerate() {
                b[0xfe + 8 * i + 2 * n..][..2].copy_from_slice(&v.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn texture_names_in_utf16_are_read() {
        // the record of a CorelDRAW 12 drawing: "Surfaces", six numbers, two colors, brightness
        let slots = [[2010, 0, 0, 0], [0, 0, 0, 0], [25, 0, 0, 0], [-55, 0, 0, 0], [-14, 0, 0, 0], [83, 0, 0, 0], [96, 96, 100, 0], [0, 0, 100, 0], [0, 0, 0, 0]];
        let f = read(&wide_record("Surfaces", &slots)).expect("a wide-named style");
        assert!(f.known && f.wide);
        assert_eq!(f.style, "Surfaces");
        assert_eq!(f.texture.seed, 2010);
        assert_eq!(f.texture.colors, vec![[245, 245, 255], [0, 0, 255]]);
        // one-byte names are not wide
        assert!(!read(&record("", "Surfaces", &slots)).unwrap().wide);
    }

    #[test]
    fn texture_settings_must_be_in_the_record() {
        let slots = [[1, 0, 0, 0]; 9];
        let mut b = wide_record("Surfaces", &slots);
        b.truncate(0xfe + 8 * 8);
        assert!(read(&b).map_or(true, |f| !f.known));
    }

    #[test]
    fn texture_styles_without_colors_are_drawn_in_greys() {
        // Cosmic Clouds: five numbers and the brightness, no color
        let slots = [[24169, 0, 0, 0], [0, 0, 0, 0], [50, 0, 0, 0], [50, 0, 0, 0], [50, 0, 0, 0], [0, 0, 0, 0]];
        let f = read(&record("", "Cosmic Clouds", &slots)).expect("a known style with no colors");
        assert!(f.known);
        assert_eq!(f.texture.colors, vec![[0; 3], [255; 3]]);
        let (rgba, w, h) = tile(&f.texture).expect("a grey tile");
        assert_eq!((w, h), (128, 128));
        assert!(rgba.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2]));
        let (min, max) = rgba.chunks(4).fold((255, 0), |(l, h), p| (p[0].min(l), p[0].max(h)));
        assert!(max - min > 100, "flat tile: {min}..{max}");
        // the brightness still applies
        let mut dark = slots;
        dark[5] = [-50, 0, 0, 0];
        assert_eq!(read(&record("", "Cosmic Clouds", &dark)).unwrap().texture.colors, vec![[0; 3], [128; 3]]);
        // Cosmic Minerals too, in a wide record
        let mut minerals = [[3i16, 0, 0, 0]; 8];
        minerals[7] = [0; 4];
        let f = read(&wide_record("Cosmic Minerals", &minerals)).unwrap();
        assert!(f.known && f.wide);
        assert_eq!(f.texture.colors, vec![[0; 3], [255; 3]]);
    }

    #[test]
    fn texture_styles_not_in_the_library_keep_their_name_and_draw_grey() {
        let f = read(&wide_record("A Style From Another Library", &[[1, 0, 0, 0]; 5])).expect("kept");
        assert!(!f.known);
        assert_eq!(f.style, "A Style From Another Library");
        let (rgba, ..) = tile(&f.texture).unwrap();
        assert!(rgba.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2]));
        // the tile depends on the record's first setting, so two of them don't look the same
        let g = read(&record("", "Yet Another Style", &[[2, 0, 0, 0]; 5])).unwrap();
        assert!(!g.known);
        assert_eq!(g.style, "Yet Another Style");
        assert_ne!(tile(&f.texture), tile(&g.texture));
        // a record with no name at all is not a texture we can tell anything about
        assert!(read(&[0u8; 200]).is_none());
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
