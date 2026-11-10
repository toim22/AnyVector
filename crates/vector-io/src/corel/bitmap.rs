//! Bitmaps stored in a CorelDRAW drawing.

use super::build::{Ctx, Loda};
use super::color;
use super::riff::Rd;
use kurbo::Affine;
use std::sync::Arc;
use vector_core::document::ImageData;
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};

const MAX_PIXELS: usize = 200_000_000;

/// One pixel block: header fields and where its rows start.
struct Block {
    model: u32,
    width: usize,
    height: usize,
    bpp: usize,
    stride: usize,
    data: usize,
    palette: Vec<[u8; 3]>,
}

/// Read a block header whose color model field is at `at`.
fn block(b: &[u8], at: usize, mask: bool) -> Option<Block> {
    let mut r = Rd::at(b, at);
    let model = r.u32();
    r.skip(4);
    let width = r.u32() as usize;
    let height = r.u32() as usize;
    r.skip(4);
    let bpp = r.u32() as usize;
    let mut stride = r.u32() as usize;
    let _size = r.u32();
    r.skip(32);
    if width == 0 || height == 0 || width.checked_mul(height)? > MAX_PIXELS || !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return None;
    }
    let min_stride = (width * bpp + 7) / 8;
    if stride < min_stride || stride > min_stride + 8 {
        stride = (min_stride + 3) & !3;
    }
    let mut palette = vec![];
    // indexed images carry their palette before the rows; gray and black-and-white do not
    if !mask && bpp < 24 && model != 5 && model != 6 {
        r.skip(2);
        let n = r.u16() as usize;
        for _ in 0..n.min(256) {
            let e = r.bytes(3);
            if e.len() == 3 {
                palette.push([e[2], e[1], e[0]]);
            }
        }
    }
    Some(Block { model, width, height, bpp, stride, data: r.p, palette })
}

impl Block {
    /// The pixel at (x, y from the top) as RGB. Rows are stored bottom first.
    fn pixel(&self, b: &[u8], x: usize, y: usize) -> [u8; 3] {
        let row = self.data + (self.height - 1 - y) * self.stride;
        let get = |o: usize| b.get(row + o).copied().unwrap_or(0);
        match self.bpp {
            32 => {
                let (a, c, d, e) = (get(x * 4), get(x * 4 + 1), get(x * 4 + 2), get(x * 4 + 3));
                if self.model == 3 || self.model == 0x11 {
                    // CMYK inks, 0..255
                    let col = color::cmyk(a as f64 / 255.0, c as f64 / 255.0, d as f64 / 255.0, e as f64 / 255.0);
                    [col.r, col.g, col.b]
                } else {
                    [d, c, a]
                }
            }
            24 => [get(x * 3 + 2), get(x * 3 + 1), get(x * 3)],
            8 => {
                let v = get(x);
                self.palette.get(v as usize).copied().unwrap_or([v, v, v])
            }
            4 => {
                let v = (get(x / 2) >> if x % 2 == 0 { 4 } else { 0 }) & 0x0f;
                self.palette.get(v as usize).copied().unwrap_or([v * 17, v * 17, v * 17])
            }
            _ => {
                let v = (get(x / 8) >> (7 - x % 8)) & 1;
                self.palette.get(v as usize).copied().unwrap_or(if v == 1 { [255, 255, 255] } else { [0, 0, 0] })
            }
        }
    }
    fn level(&self, b: &[u8], x: usize, y: usize) -> u8 {
        let row = self.data + (self.height - 1 - y) * self.stride;
        let get = |o: usize| b.get(row + o).copied().unwrap_or(0);
        match self.bpp {
            8 => get(x),
            1 => {
                if (get(x / 8) >> (7 - x % 8)) & 1 == 1 {
                    255
                } else {
                    0
                }
            }
            _ => 255,
        }
    }
}

/// A bitmap of CorelDRAW 1 and 2: its id, then a whole TIFF file.
pub(super) fn tiff(cx: &mut Ctx, b: &[u8]) {
    let Some(file) = b.get(4..) else { return };
    let id = Rd::new(b).u32();
    // the TIFF reader of the image crate, else ours for what it doesn't read (4-bit grays, fax
    // compression, strip sizes before compression)
    let decoded = match image::load_from_memory_with_format(file, image::ImageFormat::Tiff) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Some((w, h, rgba.into_raw()))
        }
        Err(_) => super::tiff::decode(file),
    };
    let Some((w, h, rgba)) = decoded else { return };
    let encoded = crate::image_io::encode_png(w, h, &rgba);
    let data = ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: "png".into(), name: format!("bitmap{id}") };
    let iid = cx.doc.add_image(data);
    cx.images.insert(id, (iid, w, h));
}

/// A `bmp ` record: the image and, when it has one, its transparency mask.
pub(super) fn bitmap(cx: &mut Ctx, b: &[u8]) {
    let mut r = Rd::new(b);
    let id = r.u32();
    let head = if cx.ver < 500 {
        14
    } else if cx.ver < 600 {
        46
    } else {
        50
    };
    let Some(main) = block(b, 4 + head, false) else { return };
    // offset of the mask block, when present
    let mask_at = if cx.ver >= 600 { Rd::at(b, 0x18).u32() as usize } else { 0 };
    let mask = if mask_at > 0 && b.get(mask_at + 8..mask_at + 10) == Some(b"RI") { block(b, mask_at + 8 + 0x0e, true) } else { None };
    let mask = mask.filter(|m| m.width == main.width && m.height == main.height);
    let (w, h) = (main.width, main.height);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let p = main.pixel(b, x, y);
            let a = mask.as_ref().map(|m| m.level(b, x, y)).unwrap_or(255);
            rgba.extend_from_slice(&[p[0], p[1], p[2], a]);
        }
    }
    let encoded = crate::image_io::encode_png(w as u32, h as u32, &rgba);
    let data = ImageData { width: w as u32, height: h as u32, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: "png".into(), name: format!("bitmap{id}") };
    let iid = cx.doc.add_image(data);
    cx.images.insert(id, (iid, w as u32, h as u32));
}

/// A bitmap object: the box it fills and the image it shows.
pub(super) fn object(cx: &mut Ctx, l: &Loda, world: Affine) -> Option<Object> {
    let a = l.arg(0x1e)?;
    let mut r = Rd::new(a);
    let x1 = cx.coord(&mut r);
    let y1 = cx.coord(&mut r);
    let x2 = cx.coord(&mut r);
    let y2 = cx.coord(&mut r);
    let image_id = if cx.wide() {
        Rd::at(a, 0x30).u32()
    } else {
        Rd::at(a, 0x18).u16() as u32
    };
    let (iid, _, _) = *cx.images.get(&image_id)?;
    let k = cx.k;
    let (w, h) = ((x2 - x1).abs() * k, (y2 - y1).abs() * k);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let xf = world * Affine::translate((x1.min(x2), y1.max(y2))) * Affine::scale_non_uniform(1.0 / k, -1.0 / k);
    let mut st = Style::default();
    st.fill = Fill::None;
    st.outline = Outline::none();
    let img = ImageObj { image: iid, w, h, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() };
    Some(Object::new(cx.doc.new_id(), ObjKind::Image(img), xf, st))
}
