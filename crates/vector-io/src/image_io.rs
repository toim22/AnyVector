//! Raster image decoding (import) and encoding (export).

use crate::{IoError, Result};
use kurbo::Affine;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::{Document, ImageData};
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};

pub fn is_image_ext(e: &str) -> bool {
    matches!(e, "png" | "jpg" | "jpeg" | "jfif" | "bmp" | "tif" | "tiff" | "webp" | "gif" | "ico" | "tga" | "jxl")
}

/// Decode an encoded image into document image data (keeps the original bytes for PNG/JPEG).
pub fn decode(bytes: &[u8], name: &str) -> Result<ImageData> {
    if is_jxl(bytes) {
        let (w, h, rgba) = decode_jxl(bytes)?;
        return Ok(ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".to_string(), name: name.to_string() });
    }
    let fmt = image::guess_format(bytes).map_err(|e| IoError::Format(e.to_string()))?;
    let img = image::load_from_memory_with_format(bytes, fmt).map_err(|e| IoError::Format(e.to_string()))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let (format, encoded) = match fmt {
        image::ImageFormat::Png => ("png".to_string(), bytes.to_vec()),
        image::ImageFormat::Jpeg => ("jpg".to_string(), bytes.to_vec()),
        _ => ("png".to_string(), vec![]),
    };
    Ok(ImageData { width: w, height: h, rgba: Arc::new(rgba.into_raw()), encoded: Arc::new(encoded), format, name: name.to_string() })
}

/// Decode any supported bitmap: the formats of the `image` crate and JPEG XL.
pub fn load_dynamic(bytes: &[u8]) -> Result<image::DynamicImage> {
    if is_jxl(bytes) {
        let (w, h, rgba) = decode_jxl(bytes)?;
        let img = image::RgbaImage::from_raw(w, h, rgba).ok_or_else(|| IoError::Format("JPEG XL: bad image size".into()))?;
        return Ok(image::DynamicImage::ImageRgba8(img));
    }
    image::load_from_memory(bytes).map_err(|e| IoError::Format(e.to_string()))
}

/// Whether the bytes start like a JPEG XL file: a bare codestream (`FF 0A`) or the ISO BMFF
/// container (its `JXL ` signature box).
pub fn is_jxl(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0x0A]) || bytes.starts_with(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A])
}

/// Decode a JPEG XL image (lossless or lossy, any bit depth, with or without alpha) to 8-bit
/// straight sRGB RGBA. An embedded ICC profile is converted to sRGB; an animation gives its
/// first frame.
pub fn decode_jxl(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let err = |e: Box<dyn std::error::Error + Send + Sync>| IoError::Format(format!("JPEG XL: {e}"));
    let read = || jxl_oxide::JxlImage::builder().read(Cursor::new(bytes)).map_err(err);
    let mut img = read()?;
    let gray = img.pixel_format().is_grayscale();
    img.set_cms(jxl_oxide::Moxcms);
    let intent = jxl_oxide::RenderingIntent::Relative;
    img.request_color_encoding(if gray { jxl_oxide::EnumColourEncoding::gray_srgb(intent) } else { jxl_oxide::EnumColourEncoding::srgb(intent) });
    let render = match img.render_frame(0) {
        Ok(r) => r,
        // a profile the colour engine cannot convert (CMYK, say): take the samples as they are
        Err(_) => read()?.render_frame(0).map_err(err)?,
    };
    let mut stream = render.stream();
    let (w, h, c) = (stream.width(), stream.height(), stream.channels() as usize);
    let mut buf = vec![0u8; w as usize * h as usize * c];
    stream.write_to_buffer(&mut buf);
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for px in buf.chunks_exact(c.max(1)) {
        match c {
            1 => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
            2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            _ => rgba.extend_from_slice(&px[..4]),
        }
    }
    Ok((w, h, rgba))
}

/// Encode straight RGBA as a lossless JPEG XL image (without the alpha channel when every
/// pixel is opaque). There is no pure-Rust lossy encoder with a permissive license yet.
pub fn encode_jxl(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace, options::EncoderOptions};
    let opaque = rgba.chunks_exact(4).all(|p| p[3] == 255);
    let (data, cs) = if opaque { (rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(), ColorSpace::RGB) } else { (rgba.to_vec(), ColorSpace::RGBA) };
    let opts = EncoderOptions::new(w as usize, h as usize, cs, BitDepth::Eight);
    let mut out = vec![];
    zune_jpegxl::JxlSimpleEncoder::new(&data, opts).encode(&mut out).map_err(|e| IoError::Format(format!("JPEG XL: {e:?}")))?;
    Ok(out)
}

/// Create an image object for a decoded image at `dpi` (top-left at the origin).
pub fn image_object(doc: &mut Document, img: ImageData, dpi: f64) -> Object {
    let w = img.width as f64 * 72.0 / dpi;
    let h = img.height as f64 * 72.0 / dpi;
    let name = img.name.clone();
    let iid = doc.add_image(img);
    let id = doc.new_id();
    let mut st = Style::default();
    st.fill = Fill::None;
    st.outline = Outline::none();
    let mut o = Object::new(id, ObjKind::Image(ImageObj { image: iid, w, h, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), Affine::IDENTITY, st);
    o.name = name;
    o
}

/// Encoded bytes for storing an image (PNG if no original encoding is kept).
pub fn encoded_bytes(img: &ImageData) -> (Vec<u8>, &'static str) {
    if !img.encoded.is_empty() {
        let ext = if img.format == "jpg" { "jpg" } else { "png" };
        return (img.encoded.to_vec(), ext);
    }
    (encode_png(img.width, img.height, &img.rgba), "png")
}

pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    if let Some(buf) = image::RgbaImage::from_raw(w, h, rgba.to_vec()) {
        let _ = buf.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png);
    }
    out
}

#[derive(Clone, Debug)]
pub struct RasterOptions {
    pub dpi: f64,
    pub antialias: bool,
    /// Background color; `None` = transparent (when the format supports it).
    pub background: Option<Color>,
    pub jpeg_quality: u8,
    /// Restrict to an area of the page (document points); `None` = whole page.
    pub area: Option<kurbo::Rect>,
    /// Explicit pixel size (overrides dpi when set).
    pub size: Option<(u32, u32)>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        RasterOptions { dpi: 300.0, antialias: true, background: Some(Color::WHITE), jpeg_quality: 85, area: None, size: None }
    }
}

/// Render a page to straight RGBA.
pub fn render_rgba(doc: &Document, page: usize, opts: &RasterOptions) -> Option<(u32, u32, Vec<u8>)> {
    let area = opts.area.unwrap_or_else(|| doc.pages.get(page).map(|p| p.rect()).unwrap_or_default());
    let dpi = match opts.size {
        Some((w, _)) if area.width() > 0.0 => w as f64 / area.width() * 72.0,
        _ => opts.dpi,
    };
    let pm = vector_render::render_page_image(doc, page, dpi, opts.background, Some(area), opts.antialias)?;
    Some((pm.width(), pm.height(), vector_render::pixmap_to_rgba(&pm)))
}

/// Export a page as a raster image; the format is chosen from the file extension.
pub fn export_raster(doc: &Document, page: usize, path: &Path, opts: &RasterOptions) -> Result<()> {
    let ext = crate::extension(path);
    let mut o = opts.clone();
    let opaque_only = matches!(ext.as_str(), "jpg" | "jpeg" | "bmp");
    if opaque_only && o.background.is_none() {
        o.background = Some(Color::WHITE);
    }
    let (w, h, rgba) = render_rgba(doc, page, &o).ok_or_else(|| IoError::Format("image too large".into()))?;
    let bytes = encode_raster(w, h, rgba, &ext, o.jpeg_quality, o.dpi)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn encode_raster(w: u32, h: u32, rgba: Vec<u8>, ext: &str, quality: u8, dpi: f64) -> Result<Vec<u8>> {
    let img = image::RgbaImage::from_raw(w, h, rgba).ok_or_else(|| IoError::Format("bad buffer".into()))?;
    let mut out = vec![];
    let mut cur = Cursor::new(&mut out);
    let err = |e: image::ImageError| IoError::Format(e.to_string());
    match ext {
        "png" => {
            image::DynamicImage::ImageRgba8(img).write_to(&mut cur, image::ImageFormat::Png).map_err(err)?;
            drop(cur);
            out = png_with_dpi(out, dpi);
        }
        "jpg" | "jpeg" => {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cur, quality.clamp(1, 100));
            enc.encode_image(&rgb).map_err(err)?;
        }
        "bmp" => {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            image::DynamicImage::ImageRgb8(rgb).write_to(&mut cur, image::ImageFormat::Bmp).map_err(err)?;
        }
        "tif" | "tiff" => image::DynamicImage::ImageRgba8(img).write_to(&mut cur, image::ImageFormat::Tiff).map_err(err)?,
        "webp" => image::DynamicImage::ImageRgba8(img).write_to(&mut cur, image::ImageFormat::WebP).map_err(err)?,
        "gif" => image::DynamicImage::ImageRgba8(img).write_to(&mut cur, image::ImageFormat::Gif).map_err(err)?,
        "jxl" => {
            drop(cur);
            out = encode_jxl(w, h, img.as_raw())?;
        }
        "ico" => {
            let dynimg = image::DynamicImage::ImageRgba8(img).resize(256, 256, image::imageops::FilterType::Lanczos3);
            dynimg.write_to(&mut cur, image::ImageFormat::Ico).map_err(err)?
        }
        e => return Err(IoError::Unsupported(e.to_string())),
    }
    Ok(out)
}

/// Insert a pHYs chunk so the PNG carries its resolution.
fn png_with_dpi(png: Vec<u8>, dpi: f64) -> Vec<u8> {
    if png.len() < 33 || dpi <= 0.0 {
        return png;
    }
    let ppm = (dpi / 0.0254).round() as u32;
    let mut chunk = vec![];
    chunk.extend_from_slice(&9u32.to_be_bytes());
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&ppm.to_be_bytes());
    body.extend_from_slice(&ppm.to_be_bytes());
    body.push(1);
    let crc = crc32(&body);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc.to_be_bytes());
    // after the IHDR chunk (8 signature + 25 IHDR)
    let mut out = png[..33].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[33..]);
    out
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// Base64 encoding (for data URIs).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small gradient with a varying alpha.
    fn sample(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let mut v = vec![];
        for y in 0..h {
            for x in 0..w {
                let a = if alpha { ((x + y) * 255 / (w + h - 2)) as u8 } else { 255 };
                v.extend_from_slice(&[(x * 255 / (w - 1)) as u8, (y * 255 / (h - 1)) as u8, ((x * y) % 256) as u8, a]);
            }
        }
        v
    }

    #[test]
    fn jxl_round_trip_is_lossless() {
        let (w, h) = (37, 21);
        let rgba = sample(w, h, false);
        let bytes = encode_raster(w, h, rgba.clone(), "jxl", 85, 300.0).unwrap();
        assert!(is_jxl(&bytes));
        let img = decode(&bytes, "a.jxl").unwrap();
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.rgba.as_slice(), rgba.as_slice());
    }

    #[test]
    fn jxl_keeps_alpha() {
        let (w, h) = (16, 9);
        let rgba = sample(w, h, true);
        let bytes = encode_jxl(w, h, &rgba).unwrap();
        let (dw, dh, back) = decode_jxl(&bytes).unwrap();
        assert_eq!((dw, dh), (w, h));
        assert_eq!(back, rgba);
        assert!(back.chunks_exact(4).any(|p| p[3] == 0) && back.chunks_exact(4).any(|p| p[3] == 255));
    }

    #[test]
    fn jxl_sixteen_bit_and_gray() {
        use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace, options::EncoderOptions};
        let encode = |data: &[u8], cs: ColorSpace, depth: BitDepth| {
            let mut out = vec![];
            zune_jpegxl::JxlSimpleEncoder::new(data, EncoderOptions::new(2, 2, cs, depth)).encode(&mut out).unwrap();
            out
        };
        // 16-bit samples (native byte order) come out scaled to 8 bits
        let rgb16: Vec<u16> = vec![0, 65535, 257 * 100, 65535, 0, 257 * 200, 257 * 7, 257 * 8, 257 * 9, 32896, 32896, 32896];
        let data: Vec<u8> = rgb16.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let (w, h, rgba) = decode_jxl(&encode(&data, ColorSpace::RGB, BitDepth::Sixteen)).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba, [0, 255, 100, 255, 255, 0, 200, 255, 7, 8, 9, 255, 128, 128, 128, 255]);
        let gray16: Vec<u8> = [0u16, 65535, 257 * 50, 257 * 250].iter().flat_map(|v| v.to_ne_bytes()).collect();
        let (_, _, rgba) = decode_jxl(&encode(&gray16, ColorSpace::Luma, BitDepth::Sixteen)).unwrap();
        assert_eq!(rgba, [0, 0, 0, 255, 255, 255, 255, 255, 50, 50, 50, 255, 250, 250, 250, 255]);
        // gray with alpha
        let (_, _, rgba) = decode_jxl(&encode(&[10, 255, 20, 0, 30, 128, 40, 64], ColorSpace::LumaA, BitDepth::Eight)).unwrap();
        assert_eq!(rgba, [10, 10, 10, 255, 20, 20, 20, 0, 30, 30, 30, 128, 40, 40, 40, 64]);
    }

    #[test]
    fn jxl_known_vector() {
        // The 42-byte modular codestream from jxl-oxide's documentation (MIT OR Apache-2.0).
        let bytes = [
            0xff, 0x0a, 0x30, 0x54, 0x10, 0x09, 0x08, 0x06, 0x01, 0x00, 0x78, 0x00, 0x4b, 0x38, 0x41, 0x3c, 0xb6, 0x3a, 0x51, 0xfe, 0x00, 0x47, 0x1e, 0xa0, 0x85, 0xb8, 0x27, 0x1a, 0x48, 0x45, 0x84, 0x1b, 0x71, 0x4f, 0xa8, 0x3e, 0x8e, 0x30, 0x03, 0x92, 0x84, 0x01,
        ];
        assert!(is_jxl(&bytes));
        assert!(is_image_ext("jxl"));        let img = decode(&bytes, "vector.jxl").unwrap();
        assert_eq!((img.width, img.height), (240, 135));
        assert_eq!(img.rgba.len(), 240 * 135 * 4);
        assert_eq!(&img.rgba[..8], &[6, 6, 6, 255, 12, 12, 12, 255]);
        assert!(img.rgba.chunks_exact(4).all(|p| p[3] == 255));
        // File > Import and File > Open take it by its extension
        let path = std::env::temp_dir().join(format!("anyvector-jxl-test-{}.jxl", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let mut doc = Document::new(100.0, 100.0);
        let objs = crate::import(&mut doc, &path).unwrap();
        assert!(matches!(objs[0].kind, ObjKind::Image(_)));
        let opened = crate::open(&path).unwrap();
        assert_eq!(opened.images.values().next().map(|i| (i.width, i.height)), Some((240, 135)));
        let _ = std::fs::remove_file(&path);
    }
}
