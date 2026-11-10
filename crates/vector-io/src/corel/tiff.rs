//! The TIFF files CorelDRAW 1 and 2 keep their bitmaps in. They are older than most TIFF readers
//! expect: 4-bit grays, CCITT modified Huffman (fax) compression, strip byte counts that give the
//! size before compression or are missing. This reads what those files hold: one or more strips,
//! uncompressed, PackBits or modified Huffman, 1 to 8 bits of gray or palette, or 8-bit RGB.

/// The image as RGBA rows, with its width and height.
pub(super) fn decode(b: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let le = match b.get(0..4)? {
        [b'I', b'I', 42, 0] => true,
        [b'M', b'M', 0, 42] => false,
        _ => return None,
    };
    let u16_at = |p: usize| b.get(p..p + 2).map(|s| if le { u16::from_le_bytes([s[0], s[1]]) } else { u16::from_be_bytes([s[0], s[1]]) });
    let u32_at = |p: usize| b.get(p..p + 4).map(|s| if le { u32::from_le_bytes([s[0], s[1], s[2], s[3]]) } else { u32::from_be_bytes([s[0], s[1], s[2], s[3]]) });
    let ifd = u32_at(4)? as usize;
    let n = u16_at(ifd)? as usize;
    // the values of a tag (short or long), in the entry or where it points
    let values = |e: usize| -> Vec<u32> {
        let (typ, count) = (u16_at(e + 2).unwrap_or(0), u32_at(e + 4).unwrap_or(0) as usize);
        let size = match typ {
            3 => 2,
            4 => 4,
            1 => 1,
            _ => return vec![],
        };
        let count = count.min(1 << 16);
        let at = if size * count <= 4 { e + 8 } else { u32_at(e + 8).unwrap_or(0) as usize };
        (0..count)
            .filter_map(|i| match size {
                1 => b.get(at + i).map(|v| *v as u32),
                2 => u16_at(at + 2 * i).map(|v| v as u32),
                _ => u32_at(at + 4 * i),
            })
            .collect()
    };
    let mut tags: std::collections::HashMap<u16, Vec<u32>> = Default::default();
    for i in 0..n.min(256) {
        let e = ifd + 2 + 12 * i;
        tags.insert(u16_at(e)?, values(e));
    }
    let one = |t: u16, default: u32| tags.get(&t).and_then(|v| v.first().copied()).unwrap_or(default);
    let (w, h) = (one(256, 0) as usize, one(257, 0) as usize);
    if w == 0 || h == 0 || w > 20_000 || h > 20_000 {
        return None;
    }
    let bits = one(258, 1) as usize;
    let samples = one(277, 1) as usize;
    let compression = one(259, 1);
    let photometric = one(262, if bits == 1 { 0 } else { 1 });
    let lsb_first = one(266, 1) == 2;
    let rows_per_strip = (one(278, h as u32) as usize).clamp(1, h);
    let offsets = tags.get(&273).cloned().unwrap_or_default();
    if offsets.is_empty() || !matches!((bits, samples), (1 | 2 | 4 | 8, 1) | (8, 3)) {
        return None;
    }
    // byte counts may be missing or larger than what is there: read on to the next strip or the
    // end of the file
    let mut starts: Vec<usize> = offsets.iter().map(|o| *o as usize).collect();
    starts.sort();
    let end_of = |o: usize| starts.iter().copied().find(|s| *s > o).unwrap_or(b.len()).min(b.len());
    let row_bytes = (w * bits * samples).div_ceil(8);
    let mut data: Vec<u8> = Vec::with_capacity(row_bytes * h);
    for (i, &o) in offsets.iter().enumerate() {
        let o = o as usize;
        let count = tags.get(&279).and_then(|c| c.get(i)).map(|c| *c as usize).unwrap_or(usize::MAX);
        let strip = b.get(o..o.saturating_add(count).min(end_of(o)))?;
        let rows = rows_per_strip.min(h - (i * rows_per_strip).min(h));
        match compression {
            1 => data.extend_from_slice(&strip[..strip.len().min(rows * row_bytes)]),
            32773 => packbits(strip, rows * row_bytes, &mut data),
            2 if bits == 1 && samples == 1 => {
                let white_is_zero = photometric == 0;
                modified_huffman(strip, w, rows, lsb_first, white_is_zero, &mut data)
            }
            _ => return None,
        }
    }
    data.resize(row_bytes * h, 0);
    let palette = tags.get(&320).filter(|p| p.len() >= 3 << bits).cloned();
    let max = ((1u32 << bits) - 1) as f64;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &data[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..w {
            if samples == 3 {
                rgba.extend_from_slice(&[row[3 * x], row[3 * x + 1], row[3 * x + 2], 255]);
                continue;
            }
            let bit = x * bits;
            let v = ((row[bit / 8] >> (8 - bits - bit % 8)) & ((1u16 << bits) - 1) as u8) as usize;
            let px = match (&palette, photometric) {
                (Some(p), 3) => {
                    let n = 1 << bits;
                    [(p[v] >> 8) as u8, (p[n + v] >> 8) as u8, (p[2 * n + v] >> 8) as u8]
                }
                (_, 0) => [(255.0 * (1.0 - v as f64 / max)).round() as u8; 3],
                _ => [(255.0 * v as f64 / max).round() as u8; 3],
            };
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    Some((w as u32, h as u32, rgba))
}

/// PackBits: a count byte, then that many bytes plus one, or one byte repeated.
fn packbits(b: &[u8], want: usize, out: &mut Vec<u8>) {
    let start = out.len();
    let mut i = 0;
    while i < b.len() && out.len() - start < want {
        let n = b[i] as i8;
        i += 1;
        match n {
            0..=127 => {
                let e = (i + n as usize + 1).min(b.len());
                out.extend_from_slice(&b[i..e]);
                i = e;
            }
            -127..=-1 => {
                if let Some(v) = b.get(i) {
                    out.extend(std::iter::repeat(*v).take((1 - n as i32) as usize));
                }
                i += 1;
            }
            _ => {}
        }
    }
    out.truncate(start + want);
}

/// Codes of the white and black runs of fax compression (ITU T.4): (code bits, length, run).
/// Runs from 64 on are make-up codes, followed by the code of the rest (0 to 63).
const WHITE: &[(u16, u8, u16)] = &[
    (0b00110101, 8, 0), (0b000111, 6, 1), (0b0111, 4, 2), (0b1000, 4, 3), (0b1011, 4, 4), (0b1100, 4, 5), (0b1110, 4, 6), (0b1111, 4, 7),
    (0b10011, 5, 8), (0b10100, 5, 9), (0b00111, 5, 10), (0b01000, 5, 11), (0b001000, 6, 12), (0b000011, 6, 13), (0b110100, 6, 14), (0b110101, 6, 15),
    (0b101010, 6, 16), (0b101011, 6, 17), (0b0100111, 7, 18), (0b0001100, 7, 19), (0b0001000, 7, 20), (0b0010111, 7, 21), (0b0000011, 7, 22), (0b0000100, 7, 23),
    (0b0101000, 7, 24), (0b0101011, 7, 25), (0b0010011, 7, 26), (0b0100100, 7, 27), (0b0011000, 7, 28), (0b00000010, 8, 29), (0b00000011, 8, 30), (0b00011010, 8, 31),
    (0b00011011, 8, 32), (0b00010010, 8, 33), (0b00010011, 8, 34), (0b00010100, 8, 35), (0b00010101, 8, 36), (0b00010110, 8, 37), (0b00010111, 8, 38), (0b00101000, 8, 39),
    (0b00101001, 8, 40), (0b00101010, 8, 41), (0b00101011, 8, 42), (0b00101100, 8, 43), (0b00101101, 8, 44), (0b00000100, 8, 45), (0b00000101, 8, 46), (0b00001010, 8, 47),
    (0b00001011, 8, 48), (0b01010010, 8, 49), (0b01010011, 8, 50), (0b01010100, 8, 51), (0b01010101, 8, 52), (0b00100100, 8, 53), (0b00100101, 8, 54), (0b01011000, 8, 55),
    (0b01011001, 8, 56), (0b01011010, 8, 57), (0b01011011, 8, 58), (0b01001010, 8, 59), (0b01001011, 8, 60), (0b00110010, 8, 61), (0b00110011, 8, 62), (0b00110100, 8, 63),
    (0b11011, 5, 64), (0b10010, 5, 128), (0b010111, 6, 192), (0b0110111, 7, 256), (0b00110110, 8, 320), (0b00110111, 8, 384), (0b01100100, 8, 448), (0b01100101, 8, 512),
    (0b01101000, 8, 576), (0b01100111, 8, 640), (0b011001100, 9, 704), (0b011001101, 9, 768), (0b011010010, 9, 832), (0b011010011, 9, 896), (0b011010100, 9, 960), (0b011010101, 9, 1024),
    (0b011010110, 9, 1088), (0b011010111, 9, 1152), (0b011011000, 9, 1216), (0b011011001, 9, 1280), (0b011011010, 9, 1344), (0b011011011, 9, 1408), (0b010011000, 9, 1472), (0b010011001, 9, 1536),
    (0b010011010, 9, 1600), (0b011000, 6, 1664), (0b010011011, 9, 1728),
];
const BLACK: &[(u16, u8, u16)] = &[
    (0b0000110111, 10, 0), (0b010, 3, 1), (0b11, 2, 2), (0b10, 2, 3), (0b011, 3, 4), (0b0011, 4, 5), (0b0010, 4, 6), (0b00011, 5, 7),
    (0b000101, 6, 8), (0b000100, 6, 9), (0b0000100, 7, 10), (0b0000101, 7, 11), (0b0000111, 7, 12), (0b00000100, 8, 13), (0b00000111, 8, 14), (0b000011000, 9, 15),
    (0b0000010111, 10, 16), (0b0000011000, 10, 17), (0b0000001000, 10, 18), (0b00001100111, 11, 19), (0b00001101000, 11, 20), (0b00001101100, 11, 21), (0b00000110111, 11, 22), (0b00000101000, 11, 23),
    (0b00000010111, 11, 24), (0b00000011000, 11, 25), (0b000011001010, 12, 26), (0b000011001011, 12, 27), (0b000011001100, 12, 28), (0b000011001101, 12, 29), (0b000001101000, 12, 30), (0b000001101001, 12, 31),
    (0b000001101010, 12, 32), (0b000001101011, 12, 33), (0b000011010010, 12, 34), (0b000011010011, 12, 35), (0b000011010100, 12, 36), (0b000011010101, 12, 37), (0b000011010110, 12, 38), (0b000011010111, 12, 39),
    (0b000001101100, 12, 40), (0b000001101101, 12, 41), (0b000011011010, 12, 42), (0b000011011011, 12, 43), (0b000001010100, 12, 44), (0b000001010101, 12, 45), (0b000001010110, 12, 46), (0b000001010111, 12, 47),
    (0b000001100100, 12, 48), (0b000001100101, 12, 49), (0b000001010010, 12, 50), (0b000001010011, 12, 51), (0b000000100100, 12, 52), (0b000000110111, 12, 53), (0b000000111000, 12, 54), (0b000000100111, 12, 55),
    (0b000000101000, 12, 56), (0b000001011000, 12, 57), (0b000001011001, 12, 58), (0b000000101011, 12, 59), (0b000000101100, 12, 60), (0b000001011010, 12, 61), (0b000001100110, 12, 62), (0b000001100111, 12, 63),
    (0b0000001111, 10, 64), (0b000011001000, 12, 128), (0b000011001001, 12, 192), (0b000001011011, 12, 256), (0b000000110011, 12, 320), (0b000000110100, 12, 384), (0b000000110101, 12, 448), (0b0000001101100, 13, 512),
    (0b0000001101101, 13, 576), (0b0000001001010, 13, 640), (0b0000001001011, 13, 704), (0b0000001001100, 13, 768), (0b0000001001101, 13, 832), (0b0000001110010, 13, 896), (0b0000001110011, 13, 960), (0b0000001110100, 13, 1024),
    (0b0000001110101, 13, 1088), (0b0000001110110, 13, 1152), (0b0000001110111, 13, 1216), (0b0000001010010, 13, 1280), (0b0000001010011, 13, 1344), (0b0000001010100, 13, 1408), (0b0000001010101, 13, 1472), (0b0000001011010, 13, 1536),
    (0b0000001011011, 13, 1600), (0b0000001100100, 13, 1664), (0b0000001100101, 13, 1728),
];
/// Make-up codes both colors share, for runs longer than 1728.
const LONG: &[(u16, u8, u16)] = &[
    (0b00000001000, 11, 1792), (0b00000001100, 11, 1856), (0b00000001101, 11, 1920), (0b000000010010, 12, 1984), (0b000000010011, 12, 2048), (0b000000010100, 12, 2112), (0b000000010101, 12, 2176),
    (0b000000010110, 12, 2240), (0b000000010111, 12, 2304), (0b000000011100, 12, 2368), (0b000000011101, 12, 2432), (0b000000011110, 12, 2496), (0b000000011111, 12, 2560),
];

/// Reads bits one at a time, from the high bit of each byte (or the low bit first).
struct Bits<'a> {
    b: &'a [u8],
    at: usize,
    lsb_first: bool,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u16> {
        let byte = *self.b.get(self.at / 8)?;
        let shift = if self.lsb_first { self.at % 8 } else { 7 - self.at % 8 };
        self.at += 1;
        Some(((byte >> shift) & 1) as u16)
    }

    /// One run code of a color: its length (make-up codes included), or None at the end.
    fn run(&mut self, table: &[(u16, u8, u16)]) -> Option<usize> {
        let mut total = 0;
        loop {
            let (mut code, mut len) = (0u16, 0u8);
            let run = loop {
                code = (code << 1) | self.bit()?;
                len += 1;
                if let Some(e) = table.iter().chain(LONG).find(|e| e.1 == len && e.0 == code) {
                    break e.2 as usize;
                }
                if len >= 13 {
                    return None;
                }
            };
            total += run;
            if run < 64 {
                return Some(total);
            }
        }
    }
}

/// Modified Huffman (TIFF compression 2): every row starts on a byte, with a white run, and
/// alternates white and black runs to its width.
fn modified_huffman(b: &[u8], w: usize, rows: usize, lsb_first: bool, white_is_zero: bool, out: &mut Vec<u8>) {
    let row_bytes = w.div_ceil(8);
    let mut bits = Bits { b, at: 0, lsb_first };
    let ink = if white_is_zero { 1u8 } else { 0 };
    for _ in 0..rows {
        let mut row = vec![if white_is_zero { 0u8 } else { 0xff }; row_bytes];
        let mut x = 0;
        let mut black = false;
        while x < w {
            let Some(run) = bits.run(if black { BLACK } else { WHITE }) else {
                out.extend(row);
                return;
            };
            let end = (x + run).min(w);
            if black {
                for p in x..end {
                    let m = 0x80 >> (p % 8);
                    if ink == 1 {
                        row[p / 8] |= m;
                    } else {
                        row[p / 8] &= !m;
                    }
                }
            }
            x = end;
            black = !black;
        }
        out.extend(row);
        bits.at = bits.at.div_ceil(8) * 8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No code is the start of another of the same color, so the tables read one way only.
    #[test]
    fn fax_codes_are_prefix_free() {
        for table in [WHITE, BLACK] {
            let all: Vec<_> = table.iter().chain(LONG).collect();
            for a in &all {
                for b in &all {
                    if a.2 != b.2 && a.1 <= b.1 {
                        assert_ne!(b.0 >> (b.1 - a.1), a.0, "{a:?} starts {b:?}");
                    }
                }
            }
        }
        assert_eq!(WHITE.len(), 64 + 27);
        assert_eq!(BLACK.len(), 64 + 27);
    }

    /// A little-endian TIFF with one strip and the given tags (short values).
    fn tiff(w: u16, h: u16, bits: u16, compression: u16, photometric: u16, data: &[u8]) -> Vec<u8> {
        let tags = [(256u16, w), (257, h), (258, bits), (259, compression), (262, photometric), (273, 0), (277, 1)];
        let mut b = b"II*\0".to_vec();
        b.extend(8u32.to_le_bytes());
        b.extend((tags.len() as u16).to_le_bytes());
        let data_at = 8 + 2 + 12 * tags.len() + 4;
        for (t, v) in tags {
            b.extend(t.to_le_bytes());
            b.extend(3u16.to_le_bytes());
            b.extend(1u32.to_le_bytes());
            b.extend((if t == 273 { data_at as u16 } else { v }).to_le_bytes());
            b.extend([0, 0]);
        }
        b.extend(0u32.to_le_bytes());
        b.extend(data);
        b
    }

    #[test]
    fn reads_4_bit_grays_and_1_bit() {
        // white is zero: 0 white, 15 black
        let (w, h, px) = decode(&tiff(2, 1, 4, 1, 0, &[0x0f])).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(&px[..], &[255, 255, 255, 255, 0, 0, 0, 255]);
        // black is zero
        let (_, _, px) = decode(&tiff(3, 1, 1, 1, 1, &[0b1010_0000])).unwrap();
        assert_eq!([px[0], px[4], px[8]], [255, 0, 255]);
    }

    #[test]
    fn reads_fax_compression() {
        // 10 pixels: 3 white (1000), 4 black (011), 3 white (1000); padded to a byte, twice
        let row = [0b1000_0111, 0b0000_0000];
        let data = [row, row].concat();
        let (w, h, px) = decode(&tiff(10, 2, 1, 2, 0, &data)).unwrap();
        assert_eq!((w, h), (10, 2));
        let row0: Vec<u8> = (0..10).map(|x| px[x * 4]).collect();
        assert_eq!(row0, [255, 255, 255, 0, 0, 0, 0, 255, 255, 255]);
        let row1: Vec<u8> = (0..10).map(|x| px[40 + x * 4]).collect();
        assert_eq!(row0, row1);
    }
}
