//! Color spaces. AnyVector keeps colors as RGB, so every space is converted: CMYK through the
//! same device formula the PDF writer uses for CMYK output (so CMYK values come back as they
//! went out), spot and DeviceN colors through their tint transforms, ICC-based spaces by
//! their number of components, Lab through D50 to sRGB.

use super::func::Function;
use super::Pdf;
use lopdf::{Dictionary, Object};
use std::rc::Rc;
use vector_core::color::Color;

#[derive(Clone, Debug)]
pub enum Cs {
    Gray,
    Rgb,
    Cmyk,
    Lab { wp: [f64; 3], range: [f64; 4] },
    Indexed { base: Box<Cs>, hival: usize, lookup: Rc<Vec<u8>> },
    Separation { name: String, alt: Box<Cs>, func: Option<Rc<Function>> },
    DeviceN { names: Vec<String>, alt: Box<Cs>, func: Option<Rc<Function>> },
    Pattern(Option<Box<Cs>>),
}

fn u8c(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A CMYK color (components 0..1) as RGB, by the device formula of the PDF writer.
pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Color {
    Color::from_cmyk(c.clamp(0.0, 1.0) * 100.0, m.clamp(0.0, 1.0) * 100.0, y.clamp(0.0, 1.0) * 100.0, k.clamp(0.0, 1.0) * 100.0)
}

impl Cs {
    pub fn ncomps(&self) -> usize {
        match self {
            Cs::Gray | Cs::Indexed { .. } | Cs::Separation { .. } => 1,
            Cs::Rgb | Cs::Lab { .. } => 3,
            Cs::Cmyk => 4,
            Cs::DeviceN { names, .. } => names.len().max(1),
            Cs::Pattern(_) => 0,
        }
    }

    /// The color a space starts with when it is selected.
    pub fn initial(&self) -> Vec<f64> {
        match self {
            Cs::Cmyk => vec![0.0, 0.0, 0.0, 1.0],
            Cs::Separation { .. } => vec![1.0],
            Cs::DeviceN { names, .. } => vec![1.0; names.len().max(1)],
            Cs::Lab { .. } => vec![0.0, 0.0, 0.0],
            other => vec![0.0; other.ncomps()],
        }
    }

    /// The decode ranges of image samples in this space.
    pub fn default_decode(&self, bpc: u32) -> Vec<f64> {
        match self {
            Cs::Indexed { .. } => vec![0.0, ((1u32 << bpc.min(16)) - 1) as f64],
            Cs::Lab { range, .. } => vec![0.0, 100.0, range[0], range[1], range[2], range[3]],
            other => (0..other.ncomps()).flat_map(|_| [0.0, 1.0]).collect(),
        }
    }

    pub fn to_color(&self, c: &[f64]) -> Color {
        let g = |i: usize| c.get(i).copied().unwrap_or(0.0);
        match self {
            Cs::Gray => {
                let v = u8c(g(0));
                Color::rgb(v, v, v)
            }
            Cs::Rgb => Color::rgb(u8c(g(0)), u8c(g(1)), u8c(g(2))),
            Cs::Cmyk => cmyk(g(0), g(1), g(2), g(3)),
            Cs::Lab { wp, .. } => lab_to_rgb(g(0), g(1), g(2), *wp),
            Cs::Indexed { base, hival, lookup } => {
                let i = (g(0).round().max(0.0) as usize).min(*hival);
                let n = base.ncomps();
                let comps: Vec<f64> = (0..n).map(|k| lookup.get(i * n + k).copied().unwrap_or(0) as f64 / 255.0).collect();
                // Lab lookups are scaled to the space's ranges
                let comps = match base.as_ref() {
                    Cs::Lab { range, .. } => vec![comps[0] * 100.0, range[0] + comps[1] * (range[1] - range[0]), range[2] + comps[2] * (range[3] - range[2])],
                    _ => comps,
                };
                base.to_color(&comps)
            }
            Cs::Separation { name, alt, func } => {
                if name == "None" {
                    return Color::TRANSPARENT;
                }
                if name == "All" {
                    let v = u8c(1.0 - g(0));
                    return Color::rgb(v, v, v);
                }
                match func {
                    Some(f) => alt.to_color(&f.eval(&[g(0)])),
                    None => {
                        let v = u8c(1.0 - g(0));
                        Color::rgb(v, v, v)
                    }
                }
            }
            Cs::DeviceN { names, alt, func } => {
                let input: Vec<f64> = (0..names.len().max(1)).map(g).collect();
                match func {
                    Some(f) => alt.to_color(&f.eval(&input)),
                    None => {
                        let v = u8c(1.0 - input.iter().copied().fold(0.0, f64::max));
                        Color::rgb(v, v, v)
                    }
                }
            }
            Cs::Pattern(base) => base.as_ref().map(|b| b.to_color(c)).unwrap_or(Color::BLACK),
        }
    }

    /// A color space from a name or an array, looking names up in the resources.
    pub fn parse(pdf: &Pdf, o: &Object, res: Option<&Dictionary>) -> Option<Cs> {
        Self::parse_depth(pdf, o, res, 0)
    }

    fn parse_depth(pdf: &Pdf, o: &Object, res: Option<&Dictionary>, depth: usize) -> Option<Cs> {
        if depth > 8 {
            return None;
        }
        let o = pdf.deref(o);
        match o {
            Object::Name(n) => match n.as_slice() {
                b"DeviceGray" | b"G" | b"CalGray" => Some(Cs::Gray),
                b"DeviceRGB" | b"RGB" | b"CalRGB" => Some(Cs::Rgb),
                b"DeviceCMYK" | b"CMYK" => Some(Cs::Cmyk),
                b"Pattern" => Some(Cs::Pattern(None)),
                _ => {
                    let d = res.and_then(|r| pdf.get_dict(r, b"ColorSpace"))?;
                    let v = pdf.get(d, n)?;
                    Self::parse_depth(pdf, v, res, depth + 1)
                }
            },
            Object::Array(a) => {
                let fam = a.first().map(|f| pdf.deref(f)).and_then(|f| f.as_name().ok())?;
                match fam {
                    b"DeviceGray" | b"G" | b"CalGray" => Some(Cs::Gray),
                    b"DeviceRGB" | b"RGB" | b"CalRGB" => Some(Cs::Rgb),
                    b"DeviceCMYK" | b"CMYK" => Some(Cs::Cmyk),
                    b"ICCBased" => {
                        let s = a.get(1).map(|s| pdf.deref(s))?;
                        let d = pdf.dict(s)?;
                        match pdf.int(d, b"N") {
                            Some(1) => Some(Cs::Gray),
                            Some(3) => Some(Cs::Rgb),
                            Some(4) => Some(Cs::Cmyk),
                            _ => d.get(b"Alternate").ok().and_then(|alt| Self::parse_depth(pdf, alt, res, depth + 1)),
                        }
                    }
                    b"Lab" => {
                        let d = a.get(1).and_then(|d| pdf.dict(pdf.deref(d)));
                        let wp = d.map(|d| pdf.nums(d, b"WhitePoint")).filter(|v| v.len() == 3).unwrap_or_else(|| vec![0.9642, 1.0, 0.8249]);
                        let r = d.map(|d| pdf.nums(d, b"Range")).filter(|v| v.len() == 4).unwrap_or_else(|| vec![-100.0, 100.0, -100.0, 100.0]);
                        Some(Cs::Lab { wp: [wp[0], wp[1], wp[2]], range: [r[0], r[1], r[2], r[3]] })
                    }
                    b"Indexed" | b"I" => {
                        let base = Self::parse_depth(pdf, a.get(1)?, res, depth + 1)?;
                        let hival = a.get(2).and_then(|h| pdf.deref(h).as_i64().ok()).unwrap_or(255).clamp(0, 4095) as usize;
                        let lk = pdf.deref(a.get(3)?);
                        let lookup = match lk {
                            Object::String(s, _) => s.clone(),
                            Object::Stream(_) => pdf.stream_data(lk).unwrap_or_default(),
                            _ => vec![],
                        };
                        Some(Cs::Indexed { base: Box::new(base), hival, lookup: Rc::new(lookup) })
                    }
                    b"Separation" => {
                        let name = a.get(1).and_then(|n| pdf.deref(n).as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default();
                        let alt = a.get(2).and_then(|x| Self::parse_depth(pdf, x, res, depth + 1)).unwrap_or(Cs::Gray);
                        let func = a.get(3).and_then(|f| Function::parse(pdf, f)).map(Rc::new);
                        Some(Cs::Separation { name, alt: Box::new(alt), func })
                    }
                    b"DeviceN" => {
                        let names: Vec<String> = a
                            .get(1)
                            .and_then(|n| pdf.deref(n).as_array().ok())
                            .map(|n| n.iter().filter_map(|x| pdf.deref(x).as_name().ok()).map(|x| String::from_utf8_lossy(x).into_owned()).collect())
                            .unwrap_or_default();
                        let alt = a.get(2).and_then(|x| Self::parse_depth(pdf, x, res, depth + 1)).unwrap_or(Cs::Gray);
                        let func = a.get(3).and_then(|f| Function::parse(pdf, f)).map(Rc::new);
                        Some(Cs::DeviceN { names, alt: Box::new(alt), func })
                    }
                    b"Pattern" => Some(Cs::Pattern(a.get(1).and_then(|b| Self::parse_depth(pdf, b, res, depth + 1)).map(Box::new))),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn lab_to_rgb(l: f64, a: f64, b: f64, wp: [f64; 3]) -> Color {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let g = |t: f64| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let (x, y, z) = (wp[0] * g(fx), wp[1] * g(fy), wp[2] * g(fz));
    // Bradford D50 -> D65, then XYZ -> linear sRGB
    let (x, y, z) = (0.9555766 * x - 0.0230393 * y + 0.0631636 * z, -0.0282895 * x + 1.0099416 * y + 0.0210077 * z, 0.0122982 * x - 0.0204830 * y + 1.3299098 * z);
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let gg = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let bb = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    let s = |c: f64| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    Color::rgb(u8c(s(r)), u8c(s(gg)), u8c(s(bb)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_spaces() {
        assert_eq!(Cs::Cmyk.to_color(&[0.0, 0.0, 0.0, 1.0]), Color::BLACK);
        assert_eq!(Cs::Cmyk.to_color(&[1.0, 0.0, 0.0, 0.0]), Color::rgb(0, 255, 255));
        assert_eq!(Cs::Gray.to_color(&[0.5]), Color::rgb(128, 128, 128));
        let idx = Cs::Indexed { base: Box::new(Cs::Rgb), hival: 1, lookup: Rc::new(vec![255, 0, 0, 0, 0, 255]) };
        assert_eq!(idx.to_color(&[1.0]), Color::rgb(0, 0, 255));
        let white = lab_to_rgb(100.0, 0.0, 0.0, [0.9642, 1.0, 0.8249]);
        assert!(white.r > 250 && white.g > 250 && white.b > 250);
    }
}
