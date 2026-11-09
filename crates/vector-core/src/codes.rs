//! QR codes and barcodes (Object ▸ Insert ▸ QR Code, Barcode and Validate Barcode).

use kurbo::{BezPath, Rect, Shape};
use serde::{Deserialize, Serialize};

/// What makes an object more than its shape: the QR code text, the barcode data, or the
/// rollover state. Kept with the object so it can be edited and validated later.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Special {
    QrCode { text: String },
    Barcode { kind: BarcodeKind, data: String },
    /// A rollover: a group of three state groups (Normal, Over, Down); `state` is shown.
    Rollover { state: u8 },
    /// An impact effect: the frame it fills and the bounds of its lines when they were made
    /// (x0, y0, x1, y1), and the seed of its random widths and spacings.
    Impact { frame: [f64; 4], bounds: [f64; 4], seed: u32 },
    /// Object ▸ Symmetry: the first `primaries` children are the drawn objects, the others their
    /// mirror copies across `lines` mirror lines through `center` (the first at `angle` degrees).
    Symmetry { lines: u32, center: [f64; 2], angle: f64, primaries: usize },
    /// Object ▸ Perspective: the perspective type, the field (x0, y0, x1, y1), the height of the
    /// horizon and the flat objects with their planes (0 orthographic, 1 top, 2 left, 3 right,
    /// 4 side).
    Perspective { kind: u8, field: [f64; 4], horizon: f64, flats: Vec<(u8, crate::Object)> },
    /// Object ▸ Insert ▸ Object: a linked file, or an embedded one (base64 `data`).
    Ole { name: String, path: String, linked: bool, data: String, icon: bool },
}

/// The QR code of `text` as one path of dark modules, `size` points square (the quiet zone
/// around it is not part of the object).
pub fn qr_path(text: &str, size: f64) -> Option<BezPath> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let n = code.width();
    let m = size / n as f64;
    let colors = code.to_colors();
    let mut bp = BezPath::new();
    for y in 0..n {
        for x in 0..n {
            if colors[y * n + x] == qrcode::Color::Dark {
                let r = Rect::new(x as f64 * m, y as f64 * m, (x + 1) as f64 * m, (y + 1) as f64 * m);
                for el in r.to_path(0.1).elements() {
                    bp.push(*el);
                }
            }
        }
    }
    Some(bp)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarcodeKind {
    Code128,
    Code39,
    Ean13,
    UpcA,
}

impl BarcodeKind {
    pub const ALL: [BarcodeKind; 4] = [BarcodeKind::Code128, BarcodeKind::Code39, BarcodeKind::Ean13, BarcodeKind::UpcA];
    pub fn name(self) -> &'static str {
        match self {
            BarcodeKind::Code128 => "Code 128",
            BarcodeKind::Code39 => "Code 39",
            BarcodeKind::Ean13 => "EAN-13",
            BarcodeKind::UpcA => "UPC-A",
        }
    }
    /// What the data may hold, shown in the wizard.
    pub fn rule(self) -> &'static str {
        match self {
            BarcodeKind::Code128 => "Any printable ASCII character (space to ~).",
            BarcodeKind::Code39 => "Digits, capital letters, space and - . $ / + %.",
            BarcodeKind::Ean13 => "12 digits (the check digit is added) or 13 digits.",
            BarcodeKind::UpcA => "11 digits (the check digit is added) or 12 digits.",
        }
    }
}

const CODE39: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
/// Code 39 patterns: 9 elements (bar, space, bar, ...), 1 = wide.
const CODE39_PAT: [&str; 44] = [
    "000110100", "100100001", "001100001", "101100000", "000110001", "100110000", "001110000", "000100101", "100100100", "001100100", "100001001", "001001001", "101001000", "000011001", "100011000", "001011000", "000001101", "100001100", "001001100", "000011100", "100000011", "001000011", "101000010", "000010011", "100010010", "001010010", "000000111", "100000110", "001000110", "000010110", "110000001", "011000001", "111000000", "010010001", "110010000", "011010000", "010000101", "110000100", "011000100", "010101000", "010100010", "010001010", "000101010",
    // start / stop (*)
    "010010100",
];

const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212", "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221", "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221", "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321", "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131", "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131", "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111", "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114", "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141", "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311", "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

const EAN_L: [&str; 10] = ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const EAN_G: [&str; 10] = ["0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001", "0001001", "0010111"];
const EAN_R: [&str; 10] = ["1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100", "1001000", "1110100"];
const EAN_PARITY: [&str; 10] = ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/// The EAN/UPC check digit of the digits (without it).
fn gtin_check(digits: &[u8]) -> u8 {
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, d)| *d as u32 * if i % 2 == 0 { 3 } else { 1 }).sum();
    ((10 - sum % 10) % 10) as u8
}

/// Check the data for the format and complete it (check digits). Returns the text printed
/// under the bars.
pub fn validate(kind: BarcodeKind, data: &str) -> Result<String, String> {
    if data.is_empty() {
        return Err("Enter the data to encode.".into());
    }
    match kind {
        BarcodeKind::Code128 => {
            if let Some(c) = data.chars().find(|c| !(' '..='~').contains(c)) {
                return Err(format!("Code 128 cannot encode \u{201c}{c}\u{201d}."));
            }
            Ok(data.to_string())
        }
        BarcodeKind::Code39 => {
            let up = data.to_uppercase();
            if let Some(c) = up.chars().find(|c| !CODE39.contains(*c)) {
                return Err(format!("Code 39 cannot encode \u{201c}{c}\u{201d}."));
            }
            Ok(up)
        }
        BarcodeKind::Ean13 | BarcodeKind::UpcA => {
            let n = if kind == BarcodeKind::Ean13 { 13 } else { 12 };
            if !data.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("{} holds digits only.", kind.name()));
            }
            let d: Vec<u8> = data.bytes().map(|b| b - b'0').collect();
            if d.len() == n - 1 {
                return Ok(format!("{data}{}", gtin_check(&d)));
            }
            if d.len() != n {
                return Err(format!("{} needs {} or {} digits.", kind.name(), n - 1, n));
            }
            let c = gtin_check(&d[..n - 1]);
            if c != d[n - 1] {
                return Err(format!("The check digit should be {c}, not {}.", d[n - 1]));
            }
            Ok(data.to_string())
        }
    }
}

/// The modules of a barcode, left to right: true = bar. Data must be validated.
pub fn modules(kind: BarcodeKind, data: &str) -> Vec<bool> {
    let mut m = vec![];
    let mut push = |s: &str, bar_first: bool| {
        // widths string (Code 128) — alternating bar/space widths
        let mut bar = bar_first;
        for ch in s.chars() {
            let w = ch.to_digit(10).unwrap_or(1) as usize;
            for _ in 0..w {
                m.push(bar);
            }
            bar = !bar;
        }
    };
    match kind {
        BarcodeKind::Code128 => {
            let vals: Vec<usize> = data.chars().map(|c| c as usize - 32).collect();
            let start = 104usize; // Start B
            let mut sum = start;
            for (i, v) in vals.iter().enumerate() {
                sum += v * (i + 1);
            }
            push(CODE128[start], true);
            for v in &vals {
                push(CODE128[*v], true);
            }
            push(CODE128[sum % 103], true);
            push(CODE128[106], true);
        }
        BarcodeKind::Code39 => {
            let text = format!("*{data}*");
            for (k, c) in text.chars().enumerate() {
                let i = if c == '*' { 43 } else { CODE39.find(c).unwrap_or(0) };
                let pat = CODE39_PAT[i];
                // wide elements are twice the narrow ones; a narrow gap between characters
                let mut widths: String = pat.chars().map(|b| if b == '1' { '2' } else { '1' }).collect();
                if k + 1 < text.len() {
                    widths.push('1');
                }
                push(&widths, true);
            }
        }
        BarcodeKind::Ean13 | BarcodeKind::UpcA => {
            let digits: Vec<usize> = data.bytes().map(|b| (b - b'0') as usize).collect();
            // UPC-A is EAN-13 with a leading 0
            let d: Vec<usize> = if kind == BarcodeKind::UpcA { std::iter::once(0).chain(digits).collect() } else { digits };
            let bits = |s: &str, m: &mut Vec<bool>| m.extend(s.chars().map(|c| c == '1'));
            bits("101", &mut m);
            let parity = EAN_PARITY[d[0]];
            for (k, p) in parity.chars().enumerate() {
                let v = d[1 + k];
                bits(if p == 'L' { EAN_L[v] } else { EAN_G[v] }, &mut m);
            }
            bits("01010", &mut m);
            for v in &d[7..13] {
                bits(EAN_R[*v], &mut m);
            }
            bits("101", &mut m);
        }
    }
    m
}

/// The bars as one path, `width` x `height` points.
pub fn barcode_path(kind: BarcodeKind, data: &str, width: f64, height: f64) -> BezPath {
    let m = modules(kind, data);
    let w = width / m.len().max(1) as f64;
    let mut bp = BezPath::new();
    let mut i = 0;
    while i < m.len() {
        if m[i] {
            let start = i;
            while i < m.len() && m[i] {
                i += 1;
            }
            let r = Rect::new(start as f64 * w, 0.0, i as f64 * w, height);
            for el in r.to_path(0.1).elements() {
                bp.push(*el);
            }
        } else {
            i += 1;
        }
    }
    bp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barcodes_validate_and_encode() {
        assert_eq!(validate(BarcodeKind::Ean13, "400638133393").unwrap(), "4006381333931");
        assert!(validate(BarcodeKind::Ean13, "4006381333932").is_err());
        assert_eq!(validate(BarcodeKind::UpcA, "03600029145").unwrap(), "036000291452");
        assert_eq!(modules(BarcodeKind::Ean13, "4006381333931").len(), 95);
        assert_eq!(modules(BarcodeKind::UpcA, "036000291452").len(), 95);
        // Code 128: start + 3 chars + check = 5 x 11 modules, stop 13
        assert_eq!(modules(BarcodeKind::Code128, "Vec").len(), 5 * 11 + 13);
        assert!(validate(BarcodeKind::Code39, "ab-1").is_ok());
        assert!(validate(BarcodeKind::Code39, "a_b").is_err());
        // Code 39: (n + 2) characters of 12 modules plus the gaps
        assert_eq!(modules(BarcodeKind::Code39, "AB").len(), 4 * 12 + 3);
        assert!(qr_path("Vector", 72.0).is_some());
    }
}
