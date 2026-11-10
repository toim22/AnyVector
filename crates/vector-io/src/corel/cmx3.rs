//! The drawing of a vector pattern in CorelDRAW X5 and later (`CMX3`, "Corel Binary Meta File").
//!
//! From X5 on (seen in files saved as X5 and as 2026) a vector pattern fill's `vpat` chunk holds
//! a `CMX3` document instead of the RIFF metafile older versions used: `CMX3`, two words, the
//! name "Corel Binary Meta File", a few bytes, then a raw deflate stream. Inflated it is a flat
//! run of records, each a four-letter tag and an id:
//!
//! - `RCRS`, `RET!`: begin and end of a resource list (the tag and the id only);
//! - `UDIM`: an instruction (begin and end of page, layer, group and object, the color context,
//!   the thumbnail): a code, a count, then per item a byte and a sized body;
//! - `ATTR`: a fill or an outline, as CorelDRAW X3's own `fild` and `outl` records without their
//!   id, told apart by the GUID at their start;
//! - `POLY`: a shape: its kind (0x59 curve, 0x5a rectangle, 0x5b ellipse), then nested sized
//!   blocks; the last holds the geometry in CMX's 32-bit units (a 254000th of an inch, y up);
//! - `DRAW`: draws a `POLY` with its `ATTR`s, in drawing order;
//! - `META`: the index at the end.
//!
//! CorelDRAW turns rotated, rounded and partial shapes into curves when it writes the pattern,
//! so only plain rectangles, whole ellipses and curves are expected. The layout was read from
//! files CorelDRAW 2026 saved; nothing here is from a specification.

use super::build::Ctx;
use super::riff::Rd;
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use std::collections::HashMap;
use std::io::Read;
use vector_core::color::Color;
use vector_core::object::{ObjKind, Object};
use vector_core::path::NodePath;
use vector_core::style::{Fill, Outline, Style};

/// The fill and outline records start with these GUIDs.
const FILL: [u8; 16] = [0xe8, 0xb4, 0x37, 0x94, 0xbf, 0x46, 0xb0, 0x6c, 0x4b, 0xbd, 0x73, 0x17, 0xe1, 0xcc, 0x5a, 0x5f];
const OUTLINE: [u8; 16] = [0x91, 0x9b, 0x5a, 0x83, 0x41, 0xb0, 0xa2, 0xee, 0x49, 0x7f, 0x94, 0x4d, 0x0f, 0xbc, 0xe2, 0x5d];

const CURVE: u32 = 0x59;
const RECTANGLE: u32 = 0x5a;
const ELLIPSE: u32 = 0x5b;

/// Ids the pattern's fills and outlines get in the drawing's tables, apart from the drawing's own.
const IDS: u32 = 0xfe00_0000;

/// Whether the bytes are a `CMX3` document.
pub(super) fn is_cmx3(b: &[u8]) -> bool {
    b.starts_with(b"CMX3")
}

/// The inflated records: the deflate stream starts after the header's name.
fn inflate(b: &[u8]) -> Option<Vec<u8>> {
    let name = Rd::at(b, 8).u32() as usize;
    let from = 12usize.checked_add(name)?;
    (from..(from + 32).min(b.len())).find_map(|at| {
        let mut out = vec![];
        let d = flate2::read::DeflateDecoder::new(&b[at..]);
        d.take(64 << 20).read_to_end(&mut out).ok()?;
        out.starts_with(b"RCRS").then_some(out)
    })
}

const TAGS: [&[u8; 4]; 7] = [b"RCRS", b"RET!", b"UDIM", b"ATTR", b"POLY", b"DRAW", b"META"];

/// The records: (tag, id, body); a record whose size cannot be followed is passed over to the
/// next known tag.
fn records(d: &[u8]) -> Vec<([u8; 4], u32, &[u8])> {
    let mut out = vec![];
    let mut p = 0usize;
    let known = |at: usize| d.get(at..at + 4).is_some_and(|t| TAGS.iter().any(|k| &k[..] == t));
    while p + 8 <= d.len() && out.len() < 1_000_000 {
        let tag = [d[p], d[p + 1], d[p + 2], d[p + 3]];
        let id = Rd::at(d, p + 4).u32();
        let next = match &tag {
            b"META" => break,
            b"RCRS" | b"RET!" => Some((p + 8, &d[p + 8..p + 8])),
            b"UDIM" => {
                let n = Rd::at(d, p + 12).u32() as usize;
                let mut q = p + 16;
                if n == 0 {
                    q += 4;
                }
                for _ in 0..n.min(64) {
                    let len = Rd::at(d, q + 1).u32() as usize;
                    q = q.saturating_add(5).saturating_add(len);
                }
                (q <= d.len()).then(|| (q, &d[p + 8..q]))
            }
            b"ATTR" | b"DRAW" => {
                let len = Rd::at(d, p + 12).u32() as usize;
                let end = (p + 16).saturating_add(len);
                (end <= d.len()).then(|| (end, &d[p + 16..end]))
            }
            b"POLY" => {
                let len = Rd::at(d, p + 12).u32() as usize;
                let end = (p + 24).saturating_add(len);
                (end <= d.len()).then(|| (end, &d[p + 8..end]))
            }
            _ => None,
        };
        match next {
            Some((q, body)) if q >= d.len() || known(q) => {
                out.push((tag, id, body));
                p = q;
            }
            _ => {
                // an unknown record, or one whose size is not understood: look for the next tag
                match (p + 1..d.len().saturating_sub(3)).find(|&i| known(i)) {
                    Some(i) => p = i,
                    None => break,
                }
            }
        }
    }
    out
}

/// A shape's geometry in file units: the last sized block of a `POLY` body (kind, size, kind,
/// size, then two words and a block of boxes, then a word and the geometry's size).
fn geometry(body: &[u8]) -> Option<(u32, &[u8])> {
    let kind = Rd::new(body).u32();
    let boxes = Rd::at(body, 28).u32() as usize;
    let at = 32usize.checked_add(boxes)?;
    let len = Rd::at(body, at + 4).u32() as usize;
    let s = at + 8;
    Some((kind, body.get(s..s.checked_add(len)?)?))
}

fn shape(kind: u32, g: &[u8]) -> Option<BezPath> {
    let mut r = Rd::new(g);
    match kind {
        CURVE => {
            let n = r.u32() as usize;
            if r.u8() != 1 || n == 0 || n > g.len() / 8 {
                return None;
            }
            let pts: Vec<Point> = (0..n).map(|_| Point::new(r.i32() as f64, r.i32() as f64)).collect();
            if r.u8() != 1 {
                return None;
            }
            let kinds = r.bytes(n);
            (kinds.len() == n).then(|| super::cmx::curve(&pts, kinds))
        }
        RECTANGLE => {
            let (cx, cy, w, h) = (r.i32() as f64, r.i32() as f64, r.i32().unsigned_abs() as f64, r.i32().unsigned_abs() as f64);
            (w > 0.0 && h > 0.0).then(|| Rect::new(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0).to_path(0.1))
        }
        ELLIPSE => {
            let (cx, cy, w, h) = (r.i32() as f64, r.i32() as f64, r.i32().unsigned_abs() as f64, r.i32().unsigned_abs() as f64);
            let (a1, a2) = (r.i32(), r.i32());
            // partial ellipses are written as curves; one that is not would be guessed at
            (w > 0.0 && h > 0.0 && a1 == a2).then(|| kurbo::Ellipse::new((cx, cy), (w / 2.0, h / 2.0), 0.0).to_path(0.1))
        }
        _ => None,
    }
}

/// The objects of a `CMX3` vector pattern, in points with y down (the drawing's own position,
/// which the fill's box refers to). Its fills and outlines go into `cx`'s tables.
pub(super) fn objects(bytes: &[u8], cx: &mut Ctx) -> Option<Vec<Object>> {
    if !is_cmx3(bytes) {
        return None;
    }
    let d = inflate(bytes)?;
    let recs = records(&d);
    let mut attrs: HashMap<u32, (bool, u32)> = HashMap::new();
    let mut polys: HashMap<u32, &[u8]> = HashMap::new();
    let mut next = IDS + cx.fills.len() as u32 + cx.outlines.len() as u32;
    for (tag, id, body) in &recs {
        match tag {
            b"ATTR" if body.len() > 24 => {
                let guid = &body[4..20];
                let fill = guid == FILL;
                if !fill && guid != OUTLINE {
                    continue;
                }
                // after a few words comes the record's size and the record: a list of properties
                // (an id and a size each), the first one 16 bytes long (the record's GUID)
                let record = |i: usize| {
                    let len = Rd::at(body, i).u32() as usize;
                    len >= 24 && len <= body.len() - i - 4 && Rd::at(body, i + 8).u32() == 16
                };
                let Some(at) = (24..body.len().saturating_sub(28)).find(|&i| record(i)) else { continue };
                let len = Rd::at(body, at).u32() as usize;
                next += 1;
                let mut rec = next.to_le_bytes().to_vec();
                rec.extend_from_slice(&body[at + 4..at + 4 + len]);
                if fill {
                    cx.fild(&rec);
                } else {
                    cx.outl(&rec);
                }
                attrs.insert(*id, (fill, next));
            }
            b"POLY" => {
                polys.insert(*id, body);
            }
            _ => {}
        }
    }
    let to_points = Affine::new([cx.k, 0.0, 0.0, -cx.k, 0.0, 0.0]);
    let mut out = vec![];
    for (tag, _, body) in &recs {
        if tag != b"DRAW" {
            continue;
        }
        let mut r = Rd::new(body);
        let poly = r.u32();
        let n = (r.u32() as usize).min(16);
        let used: Vec<u32> = (0..n).map(|_| r.u32()).collect();
        let Some((kind, g)) = polys.get(&poly).and_then(|b| geometry(b)) else { continue };
        let Some(path) = shape(kind, g).map(|p| to_points * p) else { continue };
        if path.elements().len() < 2 {
            continue;
        }
        let mut st = Style { fill: Fill::None, outline: Outline::none(), ..Default::default() };
        for a in used {
            match attrs.get(&a) {
                Some((true, id)) => st.fill = fill(cx, *id, path.bounding_box()),
                Some((false, id)) => {
                    if let Some(o) = cx.outlines.get(id) {
                        st.outline = o.clone();
                    }
                }
                None => {}
            }
        }
        // CorelDRAW never fills an open curve
        if kind == CURVE && !path.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath)) {
            st.fill = Fill::None;
        }
        out.push(Object::new(cx.doc.new_id(), ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, st));
    }
    (!out.is_empty()).then_some(out)
}

/// A pattern object's fill: uniform and fountain fills; others (a pattern inside the pattern) as
/// a neutral tone.
fn fill(cx: &Ctx, id: u32, bounds: Rect) -> Fill {
    use super::build::RawFill;
    match cx.fills.get(&id) {
        Some(RawFill::None) | None => Fill::None,
        Some(RawFill::Solid(c)) => Fill::Solid(*c),
        Some(RawFill::Gradient { kind, angle, cx: x, cy: y, pad, stops }) => super::build::fit_gradient(*kind, *angle, *x, *y, *pad, stops, bounds, Affine::IDENTITY),
        Some(_) => Fill::Solid(Color::rgb(200, 200, 200)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damaged_documents_are_passed_over() {
        let mut cx = super::super::build::tests::ctx(1500);
        assert!(objects(b"", &mut cx).is_none());
        assert!(objects(b"CMX3", &mut cx).is_none());
        let mut b = b"CMX3".to_vec();
        b.extend_from_slice(&[0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
        assert!(objects(&b, &mut cx).is_none());
        b.extend_from_slice(&[7; 40]);
        assert!(objects(&b, &mut cx).is_none());
    }

    #[test]
    fn records_and_shapes_never_read_past_their_end() {
        let mut d = b"RCRS   POLY   ".to_vec();
        d.extend_from_slice(&[0xff; 12]);
        d.extend_from_slice(b"DRAW   ");
        let recs = records(&d);
        assert_eq!(recs[0].0, *b"RCRS");
        assert!(geometry(&[0xff; 40]).is_none());
        assert!(shape(CURVE, &[0xff; 9]).is_none());
        assert!(shape(RECTANGLE, &[0; 16]).is_none());
        assert!(shape(ELLIPSE, &[1; 3]).is_none());
    }
}
