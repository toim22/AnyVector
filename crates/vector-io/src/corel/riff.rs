//! The chunk tree of a CorelDRAW drawing.
//!
//! A CorelDRAW file is a RIFF stream of nested chunks. Three layouts exist:
//! - versions 3 to 6: a plain RIFF file;
//! - versions 7 to X5: the same, with most lists deflated inside `cmpr` lists, where each chunk
//!   size is an index into a table of real sizes (X4 and X5 wrap the stream in a zip archive);
//! - versions X6 and later: a zip archive whose `content/root.dat` holds the tree, with each
//!   chunk body replaced by a 16-byte reference into one of the `content/data/*.dat` streams.

use std::io::Read;

#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub tag: [u8; 4],
    /// The list type when this is a `RIFF`/`LIST` chunk.
    pub list: Option<[u8; 4]>,
    pub data: Vec<u8>,
    pub children: Vec<Chunk>,
}

impl Chunk {
    pub fn is(&self, tag: &[u8; 4]) -> bool {
        self.list.is_none() && &self.tag == tag
    }
    pub fn is_list(&self, tag: &[u8; 4]) -> bool {
        self.list.as_ref() == Some(tag)
    }
    pub fn child(&self, tag: &[u8; 4]) -> Option<&Chunk> {
        self.children.iter().find(|c| c.is(tag))
    }
    pub fn list_child(&self, tag: &[u8; 4]) -> Option<&Chunk> {
        self.children.iter().find(|c| c.is_list(tag))
    }
    pub fn lists<'a>(&'a self, tag: &'a [u8; 4]) -> impl Iterator<Item = &'a Chunk> + 'a {
        self.children.iter().filter(move |c| c.is_list(tag))
    }
}

/// Streams a chunk body can live in (X6 and later).
pub struct External<'a> {
    pub streams: &'a [Vec<u8>],
}

const MAX_DEPTH: usize = 64;

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Inflate one zlib stream at the start of `b`; returns the data and the bytes consumed.
fn inflate(b: &[u8], size_hint: usize) -> Option<(Vec<u8>, usize)> {
    let mut d = flate2::read::ZlibDecoder::new(b);
    let mut out = Vec::with_capacity(size_hint.min(64 << 20));
    d.read_to_end(&mut out).ok()?;
    Some((out, d.total_in() as usize))
}

/// Parse the chunks in `b[start..end]` into `out`.
pub fn parse(b: &[u8], start: usize, end: usize, blocks: Option<&[u32]>, ext: Option<&External>, depth: usize, out: &mut Vec<Chunk>) {
    let end = end.min(b.len());
    let mut o = start;
    while o + 8 <= end {
        let tag = [b[o], b[o + 1], b[o + 2], b[o + 3]];
        let mut size = u32_at(b, o + 4).unwrap_or(0) as usize;
        if let Some(bl) = blocks {
            if let Some(real) = bl.get(size) {
                size = *real as usize;
            }
        }
        o += 8;
        let body_end = o.saturating_add(size).min(end);
        if (&tag == b"RIFF" || &tag == b"LIST") && body_end >= o + 4 && depth < MAX_DEPTH {
            let lt = [b[o], b[o + 1], b[o + 2], b[o + 3]];
            if &lt == b"cmpr" {
                compressed(b, o + 4, body_end, ext, depth, out);
            } else if &lt == b"stlt" {
                // the style table is one raw record, not a list of chunks
                out.push(Chunk { tag: *b"stlt", list: None, data: b[o + 4..body_end].to_vec(), children: vec![] });
            } else {
                let mut c = Chunk { tag, list: Some(lt), data: vec![], children: vec![] };
                parse(b, o + 4, body_end, blocks, ext, depth + 1, &mut c.children);
                out.push(c);
            }
        } else {
            let body = &b[o.min(end)..body_end];
            let data = match ext {
                Some(x) if body.len() == 16 => external(body, x),
                _ => body.to_vec(),
            };
            out.push(Chunk { tag, list: None, data, children: vec![] });
        }
        o = o.saturating_add(size + (size & 1));
    }
}

/// A 16-byte reference: stream index (or -1 for a value stored in place), size, offset.
fn external(body: &[u8], x: &External) -> Vec<u8> {
    let stream = u32_at(body, 0).unwrap_or(0);
    let size = u32_at(body, 4).unwrap_or(0) as usize;
    if stream == u32::MAX {
        return body[8..(8 + size).min(16)].to_vec();
    }
    let off = u32_at(body, 8).unwrap_or(0) as usize;
    match x.streams.get(stream as usize) {
        Some(s) if off <= s.len() => s[off..off.saturating_add(size).min(s.len())].to_vec(),
        _ => vec![],
    }
}

/// A `cmpr` list: two deflated blocks, the chunks and the table of their real sizes.
fn compressed(b: &[u8], start: usize, end: usize, ext: Option<&External>, depth: usize, out: &mut Vec<Chunk>) {
    // compressed size, uncompressed size, size of the block table, reserved
    let usize_hint = u32_at(b, start + 4).unwrap_or(0) as usize;
    let mut p = start + 16;
    let Some(sig) = b.get(p..p + 4) else { return };
    if sig != b"CPng" {
        return;
    }
    p += 8;
    let Some((data, used)) = b.get(p..end).and_then(|s| inflate(s, usize_hint)) else { return };
    p += used;
    let mut blocks: Option<Vec<u32>> = None;
    // the block table follows, in its own CPng block (search a little: the first stream may be padded)
    if let Some(k) = b.get(p..end).and_then(|s| s.windows(4).position(|w| w == b"CPng")) {
        if let Some((t, _)) = b.get(p + k + 8..end).and_then(|s| inflate(s, 0)) {
            blocks = Some(t.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
        }
    }
    parse(&data, 0, data.len(), blocks.as_deref(), ext, depth + 1, out);
}

/// Little-endian reader that never fails: reads past the end give zero.
#[derive(Clone)]
pub struct Rd<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Rd { b, p: 0 }
    }
    pub fn at(b: &'a [u8], p: usize) -> Self {
        Rd { b, p }
    }
    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }
    pub fn eof(&self) -> bool {
        self.p >= self.b.len()
    }
    pub fn skip(&mut self, n: usize) {
        self.p = self.p.saturating_add(n);
    }
    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let s = self.p.min(self.b.len());
        let e = self.p.saturating_add(n).min(self.b.len());
        self.p = self.p.saturating_add(n);
        &self.b[s..e]
    }
    fn arr<const N: usize>(&mut self) -> [u8; N] {
        let mut a = [0u8; N];
        let s = self.bytes(N);
        if s.len() == N {
            a.copy_from_slice(s);
        }
        a
    }
    pub fn u8(&mut self) -> u8 {
        self.arr::<1>()[0]
    }
    pub fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.arr())
    }
    pub fn i16(&mut self) -> i16 {
        i16::from_le_bytes(self.arr())
    }
    pub fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.arr())
    }
    pub fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.arr())
    }
    pub fn f64(&mut self) -> f64 {
        let v = f64::from_le_bytes(self.arr());
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    #[test]
    fn nested_lists_and_padding() {
        let mut list = b"doc ".to_vec();
        list.extend(chunk(b"abcd", &[1, 2, 3]));
        list.extend(chunk(b"efgh", &[9]));
        let mut riff = b"CDRF".to_vec();
        riff.extend(chunk(b"vrsn", &[0xdc, 5]));
        riff.extend(chunk(b"LIST", &list));
        let file = chunk(b"RIFF", &riff);
        let mut out = vec![];
        parse(&file, 0, file.len(), None, None, 0, &mut out);
        assert_eq!(out.len(), 1);
        let root = &out[0];
        assert!(root.is_list(b"CDRF"));
        assert_eq!(root.child(b"vrsn").unwrap().data, vec![0xdc, 5]);
        let doc = root.list_child(b"doc ").unwrap();
        assert_eq!(doc.children.len(), 2);
        assert_eq!(doc.child(b"abcd").unwrap().data, vec![1, 2, 3]);
        assert_eq!(doc.child(b"efgh").unwrap().data, vec![9]);
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let file = b"RIFF\xff\xff\xff\x7fCDRFvrsn\x02\x00\x00\x00\xdc".to_vec();
        let mut out = vec![];
        parse(&file, 0, file.len(), None, None, 0, &mut out);
        assert_eq!(out.len(), 1);
        let mut r = Rd::new(&[1, 2]);
        assert_eq!(r.u32(), 0);
        assert_eq!(r.f64(), 0.0);
    }

    #[test]
    fn external_reference() {
        let streams = vec![vec![10u8, 11, 12, 13, 14, 15]];
        let x = External { streams: &streams };
        let mut body = 0u32.to_le_bytes().to_vec();
        body.extend(3u32.to_le_bytes());
        body.extend(2u32.to_le_bytes());
        body.extend(0u32.to_le_bytes());
        assert_eq!(external(&body, &x), vec![12, 13, 14]);
        let mut inl = u32::MAX.to_le_bytes().to_vec();
        inl.extend(2u32.to_le_bytes());
        inl.extend([0xdc, 5, 0, 0, 0, 0, 0, 0]);
        assert_eq!(external(&inl, &x), vec![0xdc, 5]);
    }
}
