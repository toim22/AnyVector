//! `image`, `colorimage` and `imagemask`: sampled images become bitmaps (RGBA), placed with
//! the image matrix and the current transformation.

use super::gfx::{dvec, mat_read, CSpace};
use super::interp::{op, Flow, Interp, OpFn, PsErr, R};
use super::obj::*;
use kurbo::Affine;
use std::collections::HashMap;
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::ImageData;
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};

/// Images larger than this many pixels are left out.
const MAX_PIXELS: u64 = 80_000_000;

pub fn ops() -> Vec<(&'static str, OpFn)> {
    vec![
        op("image", |i| {
            if let Some(Obj::Dict(_)) = i.os.last() {
                let d = i.pop_dict()?;
                return i.image_dict(&d, false);
            }
            let src = i.pop()?;
            let m = i.pop_arr()?;
            let bps = i.pop_int()?;
            let h = i.pop_int()?;
            let w = i.pop_int()?;
            let spec = Spec { w: w.max(0) as usize, h: h.max(0) as usize, bps: bps as usize, matrix: mat_read(&m)?, space: CSpace::Gray, decode: vec![0.0, 1.0], sources: vec![src], mask: Mask::None };
            i.draw_image(spec)
        }),
        op("colorimage", |i| {
            let ncomp = i.pop_int()?.clamp(1, 4) as usize;
            let multi = i.pop_bool()?;
            let mut sources = vec![];
            for _ in 0..if multi { ncomp } else { 1 } {
                sources.push(i.pop()?);
            }
            sources.reverse();
            let m = i.pop_arr()?;
            let bps = i.pop_int()?;
            let h = i.pop_int()?;
            let w = i.pop_int()?;
            let space = match ncomp {
                1 => CSpace::Gray,
                3 => CSpace::Rgb,
                _ => CSpace::Cmyk,
            };
            let spec = Spec { w: w.max(0) as usize, h: h.max(0) as usize, bps: bps as usize, matrix: mat_read(&m)?, decode: [0.0, 1.0].repeat(ncomp), space, sources, mask: Mask::None };
            i.draw_image(spec)
        }),
        op("imagemask", |i| {
            if let Some(Obj::Dict(_)) = i.os.last() {
                let d = i.pop_dict()?;
                return i.image_dict(&d, true);
            }
            let src = i.pop()?;
            let m = i.pop_arr()?;
            let polarity = i.pop_bool()?;
            let h = i.pop_int()?;
            let w = i.pop_int()?;
            let spec = Spec { w: w.max(0) as usize, h: h.max(0) as usize, bps: 1, matrix: mat_read(&m)?, space: CSpace::Gray, decode: if polarity { vec![1.0, 0.0] } else { vec![0.0, 1.0] }, sources: vec![src], mask: Mask::Stencil };
            i.draw_image(spec)
        }),
    ]
}

enum Mask {
    None,
    /// `imagemask`: the samples say where the current color is painted.
    Stencil,
    /// ImageType 4: colors (sample ranges) that are not painted.
    Colors(Vec<i64>),
    /// ImageType 3: a separate mask (width, height, decode, source).
    Separate(usize, usize, bool, Obj),
}

struct Spec {
    w: usize,
    h: usize,
    bps: usize,
    matrix: Affine,
    space: CSpace,
    decode: Vec<f64>,
    sources: Vec<Obj>,
    mask: Mask,
}

impl Interp {
    fn image_dict(&mut self, d: &DictRef, stencil: bool) -> R {
        let kind = self.dnum(d, "ImageType").unwrap_or(1.0) as i64;
        if kind == 3 {
            let (Some(Obj::Dict(data)), Some(Obj::Dict(mask))) = (self.dget(d, "DataDict"), self.dget(d, "MaskDict")) else { return Err(PsErr::TypeCheck.into()) };
            let mut spec = self.spec_of(&data, false)?;
            let inter = self.dnum(d, "InterleaveType").unwrap_or(3.0) as i64;
            if inter == 3 {
                let mw = self.dnum(&mask, "Width").unwrap_or(0.0) as usize;
                let mh = self.dnum(&mask, "Height").unwrap_or(0.0) as usize;
                let inv = matches!(self.dget(&mask, "Decode"), Some(Obj::Array(a, _)) if a.get(0).and_then(|o| o.num()) == Some(1.0));
                let src = self.dget(&mask, "DataSource").unwrap_or(Obj::Null);
                spec.mask = Mask::Separate(mw, mh, inv, src);
            }
            return self.draw_image(spec);
        }
        let mut spec = self.spec_of(d, stencil)?;
        if kind == 4 {
            if let Some(Obj::Array(a, _)) = self.dget(d, "MaskColor") {
                spec.mask = Mask::Colors(a.items().iter().filter_map(|o| o.num()).map(|v| v as i64).collect());
            }
        }
        self.draw_image(spec)
    }

    fn spec_of(&mut self, d: &DictRef, stencil: bool) -> R<Spec> {
        let w = self.dnum(d, "Width").ok_or(Flow::Err(PsErr::TypeCheck))? as usize;
        let h = self.dnum(d, "Height").ok_or(Flow::Err(PsErr::TypeCheck))? as usize;
        let bps = if stencil { 1 } else { self.dnum(d, "BitsPerComponent").unwrap_or(8.0) as usize };
        let matrix = match self.dget(d, "ImageMatrix") {
            Some(Obj::Array(a, _)) => mat_read(&a)?,
            _ => Affine::new([w as f64, 0.0, 0.0, -(h as f64), 0.0, h as f64]),
        };
        let space = if stencil { CSpace::Gray } else { self.gs.color.space.clone() };
        let n = space.n().max(1);
        let decode = match self.dget(d, "Decode") {
            Some(Obj::Array(a, _)) => a.items().iter().filter_map(|o| o.num()).collect(),
            _ => default_decode(&space, bps, n),
        };
        let multi = matches!(self.dget(d, "MultipleDataSources"), Some(Obj::Bool(true)));
        let sources = match self.dget(d, "DataSource") {
            Some(Obj::Array(a, false)) if multi => a.items(),
            Some(o) => vec![o],
            None => return Err(PsErr::Undefined.into()),
        };
        Ok(Spec { w, h, bps, matrix, space, decode, sources, mask: if stencil { Mask::Stencil } else { Mask::None } })
    }

    /// Read `need` bytes from an image data source.
    fn read_source(&mut self, src: &Obj, need: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(need.min(1 << 26));
        match src {
            Obj::File(f, _) => {
                super::filters::ensure(f);
                let mut b = f.borrow_mut();
                let start = b.pos.min(b.data.len());
                let end = (start + need).min(b.data.len());
                out.extend_from_slice(&b.data[start..end]);
                b.pos = end;
            }
            Obj::Str(s, _) => {
                let bytes = s.bytes();
                if bytes.is_empty() {
                    return out;
                }
                while out.len() < need {
                    let take = (need - out.len()).min(bytes.len());
                    out.extend_from_slice(&bytes[..take]);
                }
            }
            p @ (Obj::Array(..) | Obj::Name(_, true) | Obj::Op(_)) => {
                let p = p.clone().with_exec(true);
                let mut calls = 0;
                while out.len() < need && calls < 10_000_000 {
                    calls += 1;
                    let base = self.os.len();
                    if self.exec_value(p.clone()).is_err() {
                        self.os.truncate(base);
                        break;
                    }
                    match self.os.pop() {
                        Some(Obj::Str(s, _)) if s.len > 0 => out.extend(s.bytes()),
                        _ => break,
                    }
                    self.os.truncate(base);
                }
            }
            _ => {}
        }
        out
    }

    fn draw_image(&mut self, spec: Spec) -> R {
        let (w, h) = (spec.w, spec.h);
        if w == 0 || h == 0 || !matches!(spec.bps, 1 | 2 | 4 | 8 | 12 | 16) {
            return Err(PsErr::RangeCheck.into());
        }
        let ncomp = if matches!(spec.mask, Mask::Stencil) { 1 } else { spec.space.n().max(1) };
        let row = (w * ncomp * spec.bps).div_ceil(8);
        let need = row * h;
        // a JPEG (DCTDecode): decoded whole, in the colors it holds
        let jpeg = match spec.sources.first() {
            Some(Obj::File(f, _)) => {
                super::filters::ensure(f);
                let b = f.borrow();
                b.dct.then(|| b.data[b.pos.min(b.data.len())..].to_vec())
            }
            _ => None,
        };
        if (w as u64) * (h as u64) > MAX_PIXELS {
            self.counts.images_skipped += 1;
            for s in spec.sources.clone() {
                let _ = self.read_source(&s, need / spec.sources.len().max(1));
            }
            return Ok(());
        }
        // a separate mask comes first in the data
        let mask_bytes = match &spec.mask {
            Mask::Separate(mw, mh, _, src) => {
                let src = src.clone();
                Some(self.read_source(&src, mw.div_ceil(8) * mh))
            }
            _ => None,
        };
        let mut encoded = vec![];
        let rgba: Vec<u8> = if let Some(j) = jpeg {
            if let Some(Obj::File(f, _)) = spec.sources.first() {
                let mut b = f.borrow_mut();
                b.pos = b.data.len();
            }
            match image::load_from_memory_with_format(&j, image::ImageFormat::Jpeg) {
                Ok(img) => {
                    let inverted = spec.decode.first().copied() == Some(1.0) && spec.decode.get(1).copied() == Some(0.0);
                    let mut px = img.to_rgba8();
                    let cmyk = matches!(spec.space, CSpace::Cmyk);
                    // Adobe's CMYK JPEGs are stored inverted; with a [1 0 ...] decode they
                    // read the right way round
                    if cmyk && inverted {
                        for p in px.pixels_mut() {
                            p.0[0] = 255 - p.0[0];
                            p.0[1] = 255 - p.0[1];
                            p.0[2] = 255 - p.0[2];
                        }
                    } else if !cmyk && !inverted {
                        encoded = j.clone();
                    }
                    if px.width() as usize != w || px.height() as usize != h {
                        encoded.clear();
                    }
                    let (pw, ph) = (px.width() as usize, px.height() as usize);
                    let mut data = px.into_raw();
                    if pw != w || ph != h {
                        data = resample(&data, pw, ph, w, h);
                    }
                    data
                }
                Err(_) => {
                    self.counts.images_skipped += 1;
                    return Ok(());
                }
            }
        } else {
            // the samples, interleaved
            let data = if spec.sources.len() > 1 {
                let per = need / spec.sources.len();
                let planes: Vec<Vec<u8>> = spec.sources.clone().iter().map(|s| self.read_source(s, per)).collect();
                let comp_row = (w * spec.bps).div_ceil(8);
                let mut bits = vec![0u8; need];
                // interleave plane samples (8-bit is by far the usual case)
                if spec.bps == 8 {
                    for y in 0..h {
                        for x in 0..w {
                            for (c, p) in planes.iter().enumerate() {
                                bits[y * row + x * ncomp + c] = p.get(y * comp_row + x).copied().unwrap_or(0);
                            }
                        }
                    }
                } else {
                    for y in 0..h {
                        for x in 0..w {
                            for (c, p) in planes.iter().enumerate() {
                                let v = sample(p, y * comp_row * 8 + x * spec.bps, spec.bps);
                                put_sample(&mut bits, y * row * 8 + (x * ncomp + c) * spec.bps, spec.bps, v);
                            }
                        }
                    }
                }
                bits
            } else {
                let src = spec.sources[0].clone();
                self.read_source(&src, need)
            };
            if data.is_empty() {
                self.counts.images_skipped += 1;
                return Ok(());
            }
            self.samples_to_rgba(&spec, &data, row, ncomp)
        };
        let mut rgba = rgba;
        if let (Mask::Separate(mw, mh, inv, _), Some(m)) = (&spec.mask, mask_bytes) {
            if *mw > 0 && *mh > 0 {
                let mrow = mw.div_ceil(8);
                for y in 0..h {
                    for x in 0..w {
                        let (mx, my) = (x * mw / w, y * mh / h);
                        let bit = (m.get(my * mrow + mx / 8).copied().unwrap_or(0) >> (7 - mx % 8)) & 1;
                        // with the default decode a 1 masks out
                        if (bit == 1) != *inv {
                            rgba[(y * w + x) * 4 + 3] = 0;
                        }
                    }
                }
            }
            encoded.clear();
        }
        if self.gs.null_device {
            return Ok(());
        }
        let ctm = self.gs.ctm;
        if spec.matrix.determinant().abs() < 1e-300 {
            return Err(PsErr::UndefinedResult.into());
        }
        let t = self.out.to_doc * ctm * spec.matrix.inverse();
        let wv = dvec(t, w as f64, 0.0).hypot();
        let hv = dvec(t, 0.0, h as f64).hypot();
        if wv < 1e-9 || hv < 1e-9 {
            return Ok(());
        }
        let obj_t = t * Affine::scale_non_uniform(w as f64 / wv, h as f64 / hv);
        let format = if encoded.is_empty() { "png" } else { "jpg" };
        let data = ImageData { width: w as u32, height: h as u32, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: format.into(), name: "PostScript image".into() };
        if self.out.capture.is_some() {
            return Ok(());
        }
        let iid = self.out.doc.add_image(data);
        let id = self.out.doc.new_id();
        let st = Style { fill: Fill::None, outline: Outline::none(), overprint_bitmap: self.gs.overprint, ..Style::default() };
        let o = Object::new(id, ObjKind::Image(ImageObj { image: iid, w: wv, h: hv, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), obj_t, st);
        let clip = self.gs.clip.clone();
        self.out.add_object(o, &clip);
        Ok(())
    }

    fn samples_to_rgba(&mut self, spec: &Spec, data: &[u8], row: usize, ncomp: usize) -> Vec<u8> {
        let (w, h, bps) = (spec.w, spec.h, spec.bps);
        let max = ((1u32 << bps.min(16)) - 1) as f64;
        let mut out = vec![0u8; w * h * 4];
        if let Mask::Stencil = spec.mask {
            let pc = self.gs.color.clone();
            let c = self.paint_color(&pc);
            let paint_on = if spec.decode.first().copied() == Some(1.0) { 1 } else { 0 };
            for y in 0..h {
                for x in 0..w {
                    let v = sample(data, y * row * 8 + x, 1);
                    if v == paint_on {
                        let k = (y * w + x) * 4;
                        out[k..k + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                    }
                }
            }
            return out;
        }
        let dec = |c: usize, v: u32| -> f64 {
            let (lo, hi) = (spec.decode.get(2 * c).copied().unwrap_or(0.0), spec.decode.get(2 * c + 1).copied().unwrap_or(1.0));
            lo + v as f64 / max * (hi - lo)
        };
        // colors of sample values seen, for the color spaces that need the interpreter
        let mut cache: HashMap<Vec<u32>, Color> = HashMap::new();
        let mask_colors = match &spec.mask {
            Mask::Colors(c) => Some(c.clone()),
            _ => None,
        };
        let space = spec.space.clone();
        let mut raw = vec![0u32; ncomp];
        let mut comps = vec![0f64; ncomp];
        for y in 0..h {
            for x in 0..w {
                for (c, r) in raw.iter_mut().enumerate() {
                    *r = if bps == 8 { data.get(y * row + x * ncomp + c).copied().unwrap_or(0) as u32 } else { sample(data, y * row * 8 + (x * ncomp + c) * bps, bps) };
                }
                let color = match &space {
                    CSpace::Gray | CSpace::Rgb => {
                        for (c, v) in comps.iter_mut().enumerate() {
                            *v = dec(c, raw[c]);
                        }
                        let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                        if ncomp == 1 {
                            Color::rgb(b(comps[0]), b(comps[0]), b(comps[0]))
                        } else {
                            Color::rgb(b(comps[0]), b(comps[1]), b(comps[2]))
                        }
                    }
                    _ => {
                        if let Some(c) = cache.get(&raw) {
                            *c
                        } else {
                            for (c, v) in comps.iter_mut().enumerate() {
                                *v = dec(c, raw[c]);
                            }
                            let col = self.rgb(&space, &comps);
                            if cache.len() < 1 << 20 {
                                cache.insert(raw.clone(), col);
                            }
                            col
                        }
                    }
                };
                let mut a = 255;
                if let Some(mc) = &mask_colors {
                    // one value or a range per component
                    let hit = if mc.len() == ncomp {
                        raw.iter().zip(mc).all(|(r, m)| *r as i64 == *m)
                    } else {
                        raw.iter().enumerate().all(|(c, r)| {
                            let (lo, hi) = (mc.get(2 * c).copied().unwrap_or(0), mc.get(2 * c + 1).copied().unwrap_or(-1));
                            (*r as i64) >= lo && (*r as i64) <= hi
                        })
                    };
                    if hit {
                        a = 0;
                    }
                }
                let k = (y * w + x) * 4;
                out[k..k + 4].copy_from_slice(&[color.r, color.g, color.b, a]);
            }
        }
        out
    }
}

fn default_decode(space: &CSpace, bps: usize, n: usize) -> Vec<f64> {
    match space {
        CSpace::Indexed { .. } => vec![0.0, ((1u32 << bps.min(16)) - 1) as f64],
        CSpace::Lab => vec![0.0, 100.0, -128.0, 127.0, -128.0, 127.0],
        _ => [0.0, 1.0].repeat(n),
    }
}

/// A sample of `bps` bits at bit offset `bit`.
fn sample(d: &[u8], bit: usize, bps: usize) -> u32 {
    match bps {
        8 => d.get(bit / 8).copied().unwrap_or(0) as u32,
        16 => {
            let k = bit / 8;
            ((d.get(k).copied().unwrap_or(0) as u32) << 8) | d.get(k + 1).copied().unwrap_or(0) as u32
        }
        _ => {
            let mut v = 0;
            for b in 0..bps {
                let at = bit + b;
                let byte = d.get(at / 8).copied().unwrap_or(0);
                v = v << 1 | ((byte >> (7 - at % 8)) & 1) as u32;
            }
            v
        }
    }
}

fn put_sample(d: &mut [u8], bit: usize, bps: usize, v: u32) {
    for b in 0..bps {
        let at = bit + b;
        let on = (v >> (bps - 1 - b)) & 1;
        if let Some(byte) = d.get_mut(at / 8) {
            if on == 1 {
                *byte |= 1 << (7 - at % 8);
            }
        }
    }
}

/// Nearest-neighbor resampling of RGBA pixels.
fn resample(d: &[u8], sw: usize, sh: usize, w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x * sw / w, y * sh / h);
            let s = (sy * sw + sx) * 4;
            let k = (y * w + x) * 4;
            if s + 4 <= d.len() {
                out[k..k + 4].copy_from_slice(&d[s..s + 4]);
            }
        }
    }
    out
}
