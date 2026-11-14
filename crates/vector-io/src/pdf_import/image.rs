//! Image XObjects and inline images: samples in any color space and bit depth, Flate/LZW/RLE
//! through lopdf, JPEG through the image crate (kept as JPEG when it can be), stencil masks,
//! soft masks and color-key masks as alpha.

use super::color::Cs;
use super::Pdf;
use lopdf::{Dictionary, Object, Stream};
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::ImageData;

/// Why an image couldn't be brought in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImageSkip {
    Jpeg2000,
    Fax,
    Jbig2,
    Broken,
}

pub const MAX_PIXELS: u64 = 120_000_000;

/// The filters of a stream, in decoding order.
fn filters(pdf: &Pdf, d: &Dictionary) -> Vec<String> {
    match d.get(b"Filter").ok().map(|f| pdf.deref(f)) {
        Some(Object::Name(n)) => vec![String::from_utf8_lossy(n).into_owned()],
        Some(Object::Array(a)) => a.iter().filter_map(|x| pdf.deref(x).as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()).collect(),
        _ => vec![],
    }
}

/// The stream data with every filter but a final image codec undone: (bytes, that codec).
fn undo_filters(pdf: &Pdf, d: &Dictionary, raw: &[u8]) -> Result<(Vec<u8>, Option<String>), ImageSkip> {
    let mut fs = filters(pdf, d);
    let codec = match fs.last().map(|s| s.as_str()) {
        Some("DCTDecode" | "JPXDecode" | "CCITTFaxDecode" | "JBIG2Decode") => fs.pop(),
        _ => None,
    };
    if fs.is_empty() {
        return Ok((raw.to_vec(), codec));
    }
    let mut nd = Dictionary::new();
    nd.set("Filter", Object::Array(fs.iter().map(|f| Object::Name(f.as_bytes().to_vec())).collect()));
    if let Ok(p) = d.get(b"DecodeParms") {
        let p = pdf.deref(p).clone();
        // the parameters of the codec are not the others' parameters
        let p = match (p, &codec) {
            (Object::Array(mut a), Some(_)) => {
                a.truncate(fs.len());
                Object::Array(a)
            }
            (Object::Dictionary(_), Some(_)) => Object::Null,
            (p, _) => p,
        };
        if !matches!(p, Object::Null) {
            nd.set("DecodeParms", p);
        }
    }
    let s = Stream::new(nd, raw.to_vec());
    let data = s.decompressed_content_with_limit(1 << 30).map_err(|_| ImageSkip::Broken)?;
    Ok((data, codec))
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    #[inline]
    fn read(&mut self, bits: u32) -> u32 {
        match bits {
            8 => {
                let v = self.d.get(self.pos / 8).copied().unwrap_or(0) as u32;
                self.pos += 8;
                v
            }
            16 => {
                let i = self.pos / 8;
                let v = ((self.d.get(i).copied().unwrap_or(0) as u32) << 8) | self.d.get(i + 1).copied().unwrap_or(0) as u32;
                self.pos += 16;
                v
            }
            _ => {
                let mut v = 0u32;
                for _ in 0..bits {
                    let byte = self.d.get(self.pos / 8).copied().unwrap_or(0);
                    v = (v << 1) | ((byte >> (7 - (self.pos % 8))) & 1) as u32;
                    self.pos += 1;
                }
                v
            }
        }
    }
    fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
}

/// Raw samples (one value per component, row by row) of an unfiltered image.
fn samples(data: &[u8], w: usize, h: usize, n: usize, bpc: u32) -> Vec<u32> {
    let mut out = Vec::with_capacity(w * h * n);
    let mut b = Bits { d: data, pos: 0 };
    for _ in 0..h {
        for _ in 0..w * n {
            out.push(b.read(bpc));
        }
        b.align();
    }
    out
}

fn get<'a>(pdf: &Pdf<'a>, d: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
    pdf.get(d, k)
}

/// A stencil mask's alpha: 255 where it paints.
fn stencil(pdf: &Pdf, d: &Dictionary, data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let decode = pdf.nums(d, b"Decode");
    let paint_on = if decode.first().copied().unwrap_or(0.0) > 0.5 { 1 } else { 0 };
    samples(data, w, h, 1, 1).into_iter().map(|s| if s == paint_on { 255 } else { 0 }).collect()
}

/// Decode an image. `fill` colors stencil masks; `res` resolves named color spaces.
pub fn decode(pdf: &Pdf, d: &Dictionary, raw: &[u8], res: Option<&Dictionary>, fill: Color, name: &str) -> Result<ImageData, ImageSkip> {
    let w = pdf.int(d, b"Width").unwrap_or(0).max(0) as usize;
    let h = pdf.int(d, b"Height").unwrap_or(0).max(0) as usize;
    if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_PIXELS {
        return Err(ImageSkip::Broken);
    }
    let (data, codec) = undo_filters(pdf, d, raw)?;
    let is_mask = get(pdf, d, b"ImageMask").and_then(|o| o.as_bool().ok()).unwrap_or(false);
    match codec.as_deref() {
        Some("JPXDecode") => return Err(ImageSkip::Jpeg2000),
        Some("CCITTFaxDecode") => return Err(ImageSkip::Fax),
        Some("JBIG2Decode") => return Err(ImageSkip::Jbig2),
        _ => {}
    }
    if is_mask {
        if codec.is_some() {
            return Err(ImageSkip::Broken);
        }
        let a = stencil(pdf, d, &data, w, h);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for v in a {
            rgba.extend_from_slice(&[fill.r, fill.g, fill.b, ((v as u32 * fill.a as u32) / 255) as u8]);
        }
        return Ok(ImageData { width: w as u32, height: h as u32, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".into(), name: name.into() });
    }
    let cs = get(pdf, d, b"ColorSpace").and_then(|c| Cs::parse(pdf, c, res));
    let decode_arr = pdf.nums(d, b"Decode");
    let has_smask = get(pdf, d, b"SMask").is_some_and(|s| matches!(s, Object::Stream(_)));
    let mask = get(pdf, d, b"Mask");
    let (mut rgba, encoded, format) = if codec.as_deref() == Some("DCTDecode") {
        let img = image::load_from_memory_with_format(&data, image::ImageFormat::Jpeg).map_err(|_| ImageSkip::Broken)?;
        let gray_or_rgb = matches!(cs, None | Some(Cs::Gray) | Some(Cs::Rgb)) && img.color().channel_count() <= 3;
        let inverted = decode_arr.first().copied().unwrap_or(0.0) > 0.5;
        let mut px = img.to_rgba8().into_raw();
        if inverted {
            for p in px.chunks_mut(4) {
                p[0] = 255 - p[0];
                p[1] = 255 - p[1];
                p[2] = 255 - p[2];
            }
        }
        let (iw, ih) = (img.width() as usize, img.height() as usize);
        if iw != w || ih != h {
            px = resample(&px, iw, ih, w, h, 4);
        }
        // spot and indexed JPEGs go through their color space
        if let Some(cs @ (Cs::Separation { .. } | Cs::DeviceN { .. } | Cs::Indexed { .. })) = &cs {
            let lut: Vec<Color> = (0..256).map(|v| cs.to_color(&[v as f64 / 255.0])).collect();
            for p in px.chunks_mut(4) {
                let c = lut[p[0] as usize];
                p[0] = c.r;
                p[1] = c.g;
                p[2] = c.b;
            }
        }
        let keep = gray_or_rgb && !inverted && !has_smask && mask.is_none() && iw == w && ih == h;
        (px, if keep { data.clone() } else { vec![] }, if keep { "jpg" } else { "png" })
    } else {
        let cs = cs.unwrap_or(Cs::Gray);
        let bpc = pdf.int(d, b"BitsPerComponent").unwrap_or(8).clamp(1, 16) as u32;
        let n = cs.ncomps().max(1);
        let s = samples(&data, w, h, n, bpc);
        let max = ((1u32 << bpc) - 1) as f64;
        let dec = if decode_arr.len() >= 2 * n { decode_arr.clone() } else { cs.default_decode(bpc) };
        let mut px = Vec::with_capacity(w * h * 4);
        let value = |s: u32, k: usize| dec[2 * k] + s as f64 * (dec[2 * k + 1] - dec[2 * k]) / max;
        if n == 1 && bpc <= 8 {
            let lut: Vec<Color> = (0..=max as u32).map(|v| cs.to_color(&[value(v, 0)])).collect();
            for v in &s {
                let c = lut[(*v as usize).min(lut.len() - 1)];
                px.extend_from_slice(&[c.r, c.g, c.b, 255]);
            }
        } else {
            let mut comps = vec![0f64; n];
            let fast = matches!(cs, Cs::Rgb | Cs::Cmyk);
            for p in s.chunks(n) {
                for k in 0..n {
                    comps[k] = value(*p.get(k).unwrap_or(&0), k);
                }
                let c = if fast && matches!(cs, Cs::Rgb) {
                    Color::rgb((comps[0].clamp(0.0, 1.0) * 255.0).round() as u8, (comps[1].clamp(0.0, 1.0) * 255.0).round() as u8, (comps[2].clamp(0.0, 1.0) * 255.0).round() as u8)
                } else {
                    cs.to_color(&comps)
                };
                px.extend_from_slice(&[c.r, c.g, c.b, 255]);
            }
        }
        // color-key mask: samples in the ranges are transparent
        if let Some(Object::Array(ranges)) = mask.map(|m| pdf.deref(m)) {
            let r: Vec<f64> = ranges.iter().filter_map(|x| pdf.deref(x).as_float().ok().map(|v| v as f64)).collect();
            if r.len() >= 2 * n {
                for (i, p) in s.chunks(n).enumerate() {
                    if (0..n).all(|k| (p[k] as f64) >= r[2 * k] && (p[k] as f64) <= r[2 * k + 1]) {
                        px[i * 4 + 3] = 0;
                    }
                }
            }
        }
        (px, vec![], "png")
    };
    // soft mask or stencil mask: alpha
    let alpha = if let Some(sm) = get(pdf, d, b"SMask").filter(|s| matches!(s, Object::Stream(_))) {
        soft_mask(pdf, sm)
    } else if let Some(m @ Object::Stream(_)) = mask.map(|m| pdf.deref(m)) {
        let Object::Stream(st) = m else { unreachable!() };
        let mw = pdf.int(&st.dict, b"Width").unwrap_or(0).max(0) as usize;
        let mh = pdf.int(&st.dict, b"Height").unwrap_or(0).max(0) as usize;
        undo_filters(pdf, &st.dict, &st.content).ok().filter(|_| mw > 0 && mh > 0).map(|(md, _)| (stencil(pdf, &st.dict, &md, mw, mh), mw, mh))
    } else {
        None
    };
    if let Some((a, aw, ah)) = alpha {
        let a = if aw != w || ah != h { resample(&a, aw, ah, w, h, 1) } else { a };
        for (i, v) in a.iter().enumerate().take(w * h) {
            rgba[i * 4 + 3] = *v;
        }
    }
    Ok(ImageData { width: w as u32, height: h as u32, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: format.into(), name: name.into() })
}

/// A soft mask's gray values: (alpha, width, height).
fn soft_mask(pdf: &Pdf, sm: &Object) -> Option<(Vec<u8>, usize, usize)> {
    let Object::Stream(st) = sm else { return None };
    let w = pdf.int(&st.dict, b"Width")?.max(0) as usize;
    let h = pdf.int(&st.dict, b"Height")?.max(0) as usize;
    if w == 0 || h == 0 || (w as u64 * h as u64) > MAX_PIXELS {
        return None;
    }
    let (data, codec) = undo_filters(pdf, &st.dict, &st.content).ok()?;
    let a: Vec<u8> = match codec.as_deref() {
        Some("DCTDecode") => {
            let img = image::load_from_memory_with_format(&data, image::ImageFormat::Jpeg).ok()?;
            let g = img.to_luma8();
            let (iw, ih) = (g.width() as usize, g.height() as usize);
            let v = g.into_raw();
            if iw != w || ih != h {
                resample(&v, iw, ih, w, h, 1)
            } else {
                v
            }
        }
        Some(_) => return None,
        None => {
            let bpc = pdf.int(&st.dict, b"BitsPerComponent").unwrap_or(8).clamp(1, 16) as u32;
            let max = ((1u32 << bpc) - 1) as f64;
            let dec = pdf.nums(&st.dict, b"Decode");
            let (d0, d1) = if dec.len() >= 2 { (dec[0], dec[1]) } else { (0.0, 1.0) };
            samples(&data, w, h, 1, bpc).into_iter().map(|s| ((d0 + s as f64 / max * (d1 - d0)).clamp(0.0, 1.0) * 255.0).round() as u8).collect()
        }
    };
    Some((a, w, h))
}

/// Nearest-neighbour resampling of `n`-channel pixels.
fn resample(px: &[u8], w: usize, h: usize, nw: usize, nh: usize, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(nw * nh * n);
    for y in 0..nh {
        let sy = (y * h / nh.max(1)).min(h.saturating_sub(1));
        for x in 0..nw {
            let sx = (x * w / nw.max(1)).min(w.saturating_sub(1));
            let i = (sy * w + sx) * n;
            out.extend_from_slice(px.get(i..i + n).unwrap_or(&[0, 0, 0, 0][..n.min(4)]));
        }
    }
    out
}

/// Expand the abbreviated keys and names of an inline image dictionary.
pub fn expand_inline(d: &Dictionary) -> Dictionary {
    let key = |k: &[u8]| -> Vec<u8> {
        match k {
            b"BPC" => b"BitsPerComponent".to_vec(),
            b"CS" => b"ColorSpace".to_vec(),
            b"D" => b"Decode".to_vec(),
            b"DP" => b"DecodeParms".to_vec(),
            b"F" => b"Filter".to_vec(),
            b"H" => b"Height".to_vec(),
            b"IM" => b"ImageMask".to_vec(),
            b"I" => b"Interpolate".to_vec(),
            b"W" => b"Width".to_vec(),
            other => other.to_vec(),
        }
    };
    let name = |n: &[u8]| -> Vec<u8> {
        match n {
            b"AHx" => b"ASCIIHexDecode".to_vec(),
            b"A85" => b"ASCII85Decode".to_vec(),
            b"LZW" => b"LZWDecode".to_vec(),
            b"Fl" => b"FlateDecode".to_vec(),
            b"RL" => b"RunLengthDecode".to_vec(),
            b"CCF" => b"CCITTFaxDecode".to_vec(),
            b"DCT" => b"DCTDecode".to_vec(),
            b"G" => b"DeviceGray".to_vec(),
            b"RGB" => b"DeviceRGB".to_vec(),
            b"CMYK" => b"DeviceCMYK".to_vec(),
            b"I" => b"Indexed".to_vec(),
            other => other.to_vec(),
        }
    };
    fn map_obj(o: &Object, name: &dyn Fn(&[u8]) -> Vec<u8>) -> Object {
        match o {
            Object::Name(n) => Object::Name(name(n)),
            Object::Array(a) => Object::Array(a.iter().map(|x| map_obj(x, name)).collect()),
            other => other.clone(),
        }
    }
    let mut out = Dictionary::new();
    for (k, v) in d.iter() {
        let k = key(k);
        let v = if k == b"Filter" || k == b"ColorSpace" { map_obj(v, &name) } else { v.clone() };
        out.set(k, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_depths() {
        assert_eq!(samples(&[0b1010_0000, 0b0100_0000], 3, 2, 1, 1), vec![1, 0, 1, 0, 1, 0]);
        assert_eq!(samples(&[0x12, 0x34], 2, 1, 2, 4), vec![1, 2, 3, 4]);
        assert_eq!(resample(&[1, 2, 3, 4], 2, 2, 4, 4, 1).len(), 16);
    }
}
