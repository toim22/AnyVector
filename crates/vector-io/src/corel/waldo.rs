//! Drawings saved by CorelDRAW 1 and 2 (files that start with "WL"). They are not RIFF files:
//! a header points at directories of records, and the objects form a tree of linked records.
//! The reader rewrites them as the chunk tree a CorelDRAW 3 file has, so one converter serves
//! both: each object becomes an `obj ` list holding a `lobj` record (fill, outline and
//! transform in the object itself, as version 3 keeps them).

use super::riff::{Chunk, Rd};
use std::collections::{HashMap, HashSet};

// argument types of the rewritten records (the version 3 ones)
const ARG_OUTLINE: u16 = 0x0a;
const ARG_FILL: u16 = 0x14;
const ARG_COORDS: u16 = 0x1e;
const ARG_TRANSFORM: u16 = 0x64;
const ARG_NAME: u16 = 0x3e8;

/// One node of the object tree.
#[derive(Clone, Copy)]
struct Node {
    next: u32,
    child: u32,
    group: bool,
    /// The record with a group's transform.
    more: u32,
}

pub(super) fn is_waldo(b: &[u8]) -> bool {
    b.len() >= 0x4c && &b[0..2] == b"WL"
}

/// The chunk tree of a CorelDRAW 1 or 2 drawing, or `None` when the file is damaged.
pub(super) fn tree(b: &[u8]) -> Option<Chunk> {
    if !is_waldo(b) {
        return None;
    }
    let ver: u16 = if b[2] <= b'e' { 100 } else { 200 };
    let mut r = Rd::at(b, 4);
    let mut offsets: Vec<usize> = (0..8).map(|_| r.u32() as usize).collect();
    r.skip(1);
    offsets.extend((0..10).map(|_| r.u32() as usize));

    // the record directories: type, id, offset
    let mut nodes_at: Vec<(u32, usize)> = vec![];
    let mut records: HashMap<(u8, u32), usize> = HashMap::new();
    for &d in [offsets[3], offsets[5], offsets[11]].iter().filter(|d| **d != 0 && **d < b.len()) {
        let mut r = Rd::at(b, d);
        let n = r.u16() as usize;
        for _ in 0..n.min(r.left() / 9) {
            let (t, id, off) = (r.u8(), r.u32(), r.u32() as usize);
            if off >= b.len() {
                continue;
            }
            if t == 1 {
                nodes_at.push((id, off));
            } else {
                records.insert((t, id), off);
            }
        }
    }
    let mut nodes: HashMap<u32, Node> = HashMap::new();
    for (id, off) in nodes_at {
        let mut r = Rd::at(b, off);
        if r.u32() != 0x18 {
            continue;
        }
        let next = r.u16() as u32;
        let _previous = r.u16();
        let child = r.u16() as u32;
        let _parent = r.u16();
        r.skip(4);
        let more = r.u16() as u32;
        r.skip(8); // the bounding box
        let flags = r.u16();
        nodes.insert(id, Node { next, child, group: flags & 1 != 0, more });
    }

    let mut cx = Rewrite { b, nodes: &nodes, records: &records, seen: HashSet::new() };
    let page_node = *nodes.get(&0)?;
    let mut layer = Chunk { tag: *b"LIST", list: Some(*b"layr"), data: vec![], children: vec![] };
    layer.children.push(leaf(b"lobj", loda(0x10, &[(ARG_NAME, b"Layer 1\0".to_vec())])));
    if page_node.group {
        cx.siblings(page_node.child, &mut layer.children, 0);
    }
    let gobj = list(b"gobj", vec![layer]);
    let page = list(b"page", vec![gobj]);

    let mut root = Chunk { tag: *b"RIFF", list: Some(if ver == 100 { *b"CDR1" } else { *b"CDR2" }), data: vec![], children: vec![] };
    root.children.push(leaf(b"vrsn", ver.to_le_bytes().to_vec()));
    // the page setup: the page's corners
    if let Some(m) = b.get(offsets[0]..offsets[0].saturating_add(10)) {
        root.children.push(list(b"doc ", vec![leaf(b"mcfg", m.to_vec())]));
    }
    root.children.push(page);
    Some(root)
}

struct Rewrite<'a> {
    b: &'a [u8],
    nodes: &'a HashMap<u32, Node>,
    records: &'a HashMap<(u8, u32), usize>,
    seen: HashSet<u32>,
}

impl Rewrite<'_> {
    /// Rewrite the chain of objects that starts at `id`.
    fn siblings(&mut self, mut id: u32, out: &mut Vec<Chunk>, depth: usize) {
        while depth < 64 && self.seen.insert(id) {
            let Some(n) = self.nodes.get(&id).copied() else { return };
            if n.group {
                let mut g = Chunk { tag: *b"LIST", list: Some(*b"grp "), data: vec![], children: vec![] };
                let t = self.group_transform(n.more).unwrap_or(IDENTITY);
                g.children.push(leaf(b"lobj", loda(0x0f, &[(ARG_TRANSFORM, transform_arg(t))])));
                self.siblings(n.child, &mut g.children, depth + 1);
                out.push(g);
            } else if let Some(o) = self.object(n.child) {
                out.push(o);
            }
            if n.next == 0 {
                return;
            }
            id = n.next;
        }
    }

    /// A group's transform: the last 24 bytes of the 0x8c part of its extra record.
    fn group_transform(&self, more: u32) -> Option<[f64; 6]> {
        let off = *self.records.get(&(7, more))?;
        let mut r = Rd::at(self.b, off + 0x12);
        if r.u16() != 0x8c {
            return None;
        }
        r.skip(4);
        let len = r.u32() as usize;
        if !(24..=0x100).contains(&len) {
            return None;
        }
        r.skip(len - 24);
        let m = [fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r)];
        Some(m)
    }

    /// One object record: its transform, its type and where its shape, outline and fill are.
    fn object(&self, rec: u32) -> Option<Chunk> {
        let off = *self.records.get(&(2, rec))?;
        let mut r = Rd::at(self.b, off);
        let len = r.u32() as usize;
        let start = off + 4;
        let end = start.saturating_add(len).min(self.b.len());
        let body = self.b.get(start..end)?;
        let mut r = Rd::new(body);
        let x0 = r.i16() as f64;
        let y0 = r.i16() as f64;
        let (a, b_, dx, c, d, dy) = (fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r), fixed(&mut r));
        let t = [a, b_, x0 + dx, c, d, y0 + dy];
        let kind = r.u8() as u16;
        let shape = r.u16() as usize;
        let outline = r.u16() as usize;
        let fill = r.u16() as usize;
        // text (3) is not read: CorelDRAW 1 and 2 keep it in a layout not known yet
        if kind > 4 || kind == 3 || shape == 0 || shape >= body.len() {
            return None;
        }
        // each part runs to the next one
        let part = |o: usize| {
            let e = [shape, outline, fill].into_iter().filter(|x| *x > o).min().unwrap_or(body.len()).min(body.len());
            body.get(o..e).map(|s| s.to_vec()).unwrap_or_default()
        };
        let mut args = vec![(ARG_COORDS, part(shape)), (ARG_TRANSFORM, transform_arg(t))];
        // no outline record: no outline, as CorelDRAW 1 draws it
        args.push((ARG_OUTLINE, if outline != 0 { part(outline) } else { vec![0] }));
        if fill != 0 {
            args.push((ARG_FILL, part(fill)));
        }
        Some(list(b"obj ", vec![leaf(b"lobj", loda(kind, &args))]))
    }
}

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// A 16.16 fixed-point number.
fn fixed(r: &mut Rd) -> f64 {
    r.i32() as f64 / 65536.0
}

/// A transform record as version 3 stores it: a header whose word at 0x0a points at the
/// matrix a, b, x0, c, d, y0 (16.16 fixed, offsets in whole units).
fn transform_arg(m: [f64; 6]) -> Vec<u8> {
    let mut v = vec![0u8; 12];
    v[0x0a..0x0c].copy_from_slice(&12u16.to_le_bytes());
    let fx = |x: f64| ((x * 65536.0).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32).to_le_bytes();
    let whole = |x: f64| (x.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32).to_le_bytes();
    v.extend(fx(m[0]));
    v.extend(fx(m[1]));
    v.extend(whole(m[2]));
    v.extend(fx(m[3]));
    v.extend(fx(m[4]));
    v.extend(whole(m[5]));
    v
}

/// An argument table with 16-bit fields: length, count, where the offsets and the types
/// start, the object type; then the offsets, the types (last first) and the arguments.
fn loda(kind: u16, args: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let n = args.len();
    let head = 10 + 4 * n;
    let total = head + args.iter().map(|a| a.1.len()).sum::<usize>();
    let mut v: Vec<u8> = vec![];
    for x in [total as u16, n as u16, 10, (10 + 2 * n) as u16, kind] {
        v.extend(x.to_le_bytes());
    }
    let mut at = head;
    for a in args {
        v.extend((at as u16).to_le_bytes());
        at += a.1.len();
    }
    for a in args.iter().rev() {
        v.extend(a.0.to_le_bytes());
    }
    for a in args {
        v.extend(&a.1);
    }
    v
}

fn leaf(tag: &[u8; 4], data: Vec<u8>) -> Chunk {
    Chunk { tag: *tag, list: None, data, children: vec![] }
}

fn list(t: &[u8; 4], children: Vec<Chunk>) -> Chunk {
    Chunk { tag: *b"LIST", list: Some(*t), data: vec![], children }
}

#[cfg(test)]
mod tests {
    use vector_core::object::ObjKind;

    /// A CorelDRAW 1 file with one red rectangle, 1 by 0.5 inch, 2 inches right of the center.
    fn one_rectangle() -> Vec<u8> {
        let mut b = vec![0u8; 77];
        b[0..4].copy_from_slice(b"WLe\0");
        let put32 = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        // page setup: the page's corners, thousandths of an inch
        let mcfg = b.len();
        for v in [1i16, -4250, 5500, 4250, -5500] {
            b.extend(v.to_le_bytes());
        }
        // the object record: transform, type, where the shape, outline and fill are
        let mut rec: Vec<u8> = vec![];
        rec.extend(2000i16.to_le_bytes());
        rec.extend(0i16.to_le_bytes());
        for v in [65536i32, 0, 0, 0, 65536, 0] {
            rec.extend(v.to_le_bytes());
        }
        rec.push(0); // rectangle
        let (outline, fill, shape) = (35u16, 69u16, 75u16);
        for v in [shape, outline, fill] {
            rec.extend(v.to_le_bytes());
        }
        // outline: none
        rec.extend([0u8; 34]);
        // fill: solid, CMYK 0 100 100 0
        rec.extend([1, 1, 0, 100, 100, 0]);
        // rectangle: width, height, corner radius
        for v in [1000i16, 500, 0] {
            rec.extend(v.to_le_bytes());
        }
        let rec_at = b.len();
        b.extend((rec.len() as u32).to_le_bytes());
        b.extend(&rec);
        // the tree: the page (a group) holds one object
        let node = |next: u16, child: u16, flags: u16| {
            let mut n = 0x18u32.to_le_bytes().to_vec();
            for v in [next, 0, child, 0, 0, 0, 0] {
                n.extend(v.to_le_bytes());
            }
            n.extend([0u8; 8]);
            n.extend(flags.to_le_bytes());
            n
        };
        let page_at = b.len();
        b.extend(node(0, 5, 1));
        let obj_at = b.len();
        b.extend(node(0, 10, 0));
        let dir = b.len();
        b.extend(3u16.to_le_bytes());
        for (t, id, at) in [(1u8, 0u32, page_at), (1, 5, obj_at), (2, 10, rec_at)] {
            b.push(t);
            b.extend(id.to_le_bytes());
            b.extend((at as u32).to_le_bytes());
        }
        put32(&mut b, 4, mcfg as u32);
        put32(&mut b, 4 + 3 * 4, dir as u32);
        b
    }

    #[test]
    fn opens_a_coreldraw_1_drawing() {
        let doc = super::super::load_from_bytes(&one_rectangle()).unwrap();
        let page = &doc.pages[0];
        assert!((page.width - 612.0).abs() < 0.01 && (page.height - 792.0).abs() < 0.01);
        let objects: Vec<_> = page.layers.iter().flat_map(|l| l.objects.iter()).collect();
        assert_eq!(objects.len(), 1);
        let o = objects[0];
        assert!(matches!(o.kind, ObjKind::Rect(_)), "{:?}", o.kind.type_name());
        let bb = o.bounds();
        assert!((bb.width() - 72.0).abs() < 0.01 && (bb.height() - 36.0).abs() < 0.01, "{bb:?}");
        // 2 inches right of the page's center
        assert!((bb.x0 - (306.0 + 144.0)).abs() < 0.01, "{bb:?}");
        match &o.style.fill {
            vector_core::style::Fill::Solid(c) => assert!(c.r > 200 && c.g < 60 && c.b < 60, "{c:?}"),
            f => panic!("fill {f:?}"),
        }
        assert!(o.style.outline.color.is_none());
    }
}
