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
    matches!(e, "png" | "jpg" | "jpeg" | "jfif" | "bmp" | "tif" | "tiff" | "webp" | "gif" | "ico" | "tga")
}

/// Decode an encoded image into document image data (keeps the original bytes for PNG/JPEG).
pub fn decode(bytes: &[u8], name: &str) -> Result<ImageData> {
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
