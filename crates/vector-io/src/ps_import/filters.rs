//! Decoding filters: ASCIIHex, ASCII85, RunLength, LZW, Flate (with predictors), DCT (kept as
//! JPEG for the image operators), SubFileDecode and ReusableStreamDecode.
//!
//! A filter decodes all its data at its first read: by then the program has read the
//! operators that come before the data in the file.

use super::obj::{FileRef, FilterSource, PsFile};
use std::io::Read;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Params {
    pub predictor: i64,
    pub colors: usize,
    pub bpc: usize,
    pub columns: usize,
    pub early_change: i64,
    pub eod_count: i64,
    pub eod_string: Vec<u8>,
}

impl Default for Params {
    fn default() -> Self {
        Params { predictor: 1, colors: 1, bpc: 8, columns: 1, early_change: 1, eod_count: -1, eod_string: vec![] }
    }
}

/// Filters this module decodes.
pub fn known(name: &str) -> bool {
    matches!(
        name,
        "ASCIIHexDecode" | "ASCII85Decode" | "RunLengthDecode" | "LZWDecode" | "FlateDecode" | "DCTDecode" | "SubFileDecode" | "ReusableStreamDecode" | "NullEncode"
    )
}

/// Decode a filter's data if it isn't yet.
pub fn ensure(f: &FileRef) {
    let pending = f.borrow_mut().pending.take();
    let Some(flt) = pending else { return };
    let (out, dct) = match &flt.source {
        FilterSource::File(src) => {
            ensure(src);
            let (data, pos) = {
                let b = src.borrow();
                (b.data.clone(), b.pos)
            };
            let rest = &data[pos.min(data.len())..];
            let (out, used) = decode(&flt.name, &flt.params, rest);
            src.borrow_mut().pos = pos + used;
            (out, flt.name == "DCTDecode")
        }
        FilterSource::Bytes(b) => (decode(&flt.name, &flt.params, b).0, flt.name == "DCTDecode"),
    };
    let mut fm = f.borrow_mut();
    fm.data = Rc::from(out);
    fm.pos = 0;
    fm.dct = dct;
}

/// Decode `src` with a filter: the bytes, and how many bytes of `src` the filter read.
pub fn decode(name: &str, p: &Params, src: &[u8]) -> (Vec<u8>, usize) {
    match name {
        "ASCIIHexDecode" => ascii_hex(src),
        "ASCII85Decode" => {
            // a leading `<~` is allowed
            let mut i = 0;
            while i < src.len() && src[i].is_ascii_whitespace() {
                i += 1;
            }
            if src[i..].starts_with(b"<~") {
                let (o, u) = ascii85(&src[i + 2..]);
                (o, i + 2 + u)
            } else {
                ascii85(src)
            }
        }
        "RunLengthDecode" => run_length(src),
        "LZWDecode" => {
            let (o, u) = lzw(src, p.early_change != 0);
            (predict(o, p), u)
        }
        "FlateDecode" => {
            let (o, u) = flate(src);
            (predict(o, p), u)
        }
        "DCTDecode" => {
            let n = jpeg_end(src);
            (src[..n].to_vec(), n)
        }
        "SubFileDecode" => sub_file(src, p),
        // reads the whole source (normally a filter that ends itself)
        _ => (src.to_vec(), src.len()),
    }
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Hex digits up to `>` (white space and anything else skipped): the bytes and the length read,
/// the `>` included.
pub fn ascii_hex(d: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(d.len() / 2);
    let mut hi: Option<u8> = None;
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        i += 1;
        if c == b'>' {
            break;
        }
        if let Some(v) = hex(c) {
            match hi.take() {
                None => hi = Some(v),
                Some(h) => out.push(h << 4 | v),
            }
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
    }
    (out, i)
}

/// ASCII85 up to `~>`: the bytes and the length read, the `~>` included.
pub fn ascii85(d: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(d.len() * 4 / 5);
    let mut group = [0u32; 5];
    let mut n = 0;
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        i += 1;
        match c {
            b'~' => {
                if d.get(i) == Some(&b'>') {
                    i += 1;
                }
                break;
            }
            b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                group[n] = (c - b'!') as u32;
                n += 1;
                if n == 5 {
                    let v = group.iter().fold(0u64, |a, &g| a * 85 + g as u64) as u32;
                    out.extend_from_slice(&v.to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for g in group.iter_mut().skip(n) {
            *g = 84;
        }
        let v = group.iter().fold(0u64, |a, &g| a * 85 + g as u64) as u32;
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    (out, i)
}

fn run_length(d: &[u8]) -> (Vec<u8>, usize) {
    let mut out = vec![];
    let mut i = 0;
    while i < d.len() {
        let n = d[i];
        i += 1;
        match n {
            128 => break,
            0..=127 => {
                let len = n as usize + 1;
                let end = (i + len).min(d.len());
                out.extend_from_slice(&d[i..end]);
                i = end;
            }
            _ => {
                if let Some(&b) = d.get(i) {
                    out.extend(std::iter::repeat_n(b, 257 - n as usize));
                }
                i += 1;
            }
        }
    }
    (out, i.min(d.len()))
}

fn lzw(d: &[u8], early: bool) -> (Vec<u8>, usize) {
    use weezl::{decode::Decoder, BitOrder, LzwStatus};
    let mut dec = if early { Decoder::with_tiff_size_switch(BitOrder::Msb, 8) } else { Decoder::new(BitOrder::Msb, 8) };
    let mut out = vec![];
    let mut buf = vec![0u8; 1 << 16];
    let mut at = 0;
    loop {
        let r = dec.decode_bytes(&d[at..], &mut buf);
        at += r.consumed_in;
        out.extend_from_slice(&buf[..r.consumed_out]);
        match r.status {
            Ok(LzwStatus::Ok) => {
                if r.consumed_in == 0 && r.consumed_out == 0 {
                    break;
                }
            }
            Ok(LzwStatus::Done) | Ok(LzwStatus::NoProgress) | Err(_) => break,
        }
    }
    (out, at)
}

fn flate(d: &[u8]) -> (Vec<u8>, usize) {
    let mut z = flate2::Decompress::new(true);
    let mut out = Vec::with_capacity(d.len() * 3);
    loop {
        if out.capacity() - out.len() < 1 << 15 {
            out.reserve(1 << 16);
        }
        let before = (z.total_in(), z.total_out());
        match z.decompress_vec(&d[z.total_in() as usize..], &mut out, flate2::FlushDecompress::None) {
            Ok(flate2::Status::StreamEnd) => break,
            Ok(_) => {
                if (z.total_in(), z.total_out()) == before {
                    break;
                }
            }
            Err(_) => {
                // raw deflate without the zlib header
                if z.total_in() == 0 {
                    let mut r = flate2::read::DeflateDecoder::new(d);
                    let mut o = vec![];
                    let _ = r.read_to_end(&mut o);
                    let used = d.len() - r.into_inner().len();
                    return (o, used);
                }
                break;
            }
        }
    }
    (out, z.total_in() as usize)
}

/// PNG and TIFF predictors.
fn predict(d: Vec<u8>, p: &Params) -> Vec<u8> {
    if p.predictor < 2 {
        return d;
    }
    let bpp = ((p.colors * p.bpc).div_ceil(8)).max(1);
    let row = (p.columns * p.colors * p.bpc).div_ceil(8);
    if row == 0 {
        return d;
    }
    if p.predictor == 2 {
        if p.bpc != 8 {
            return d;
        }
        let mut d = d;
        for r in d.chunks_mut(row) {
            for i in bpp..r.len() {
                r[i] = r[i].wrapping_add(r[i - bpp]);
            }
        }
        return d;
    }
    let mut out = Vec::with_capacity(d.len());
    let mut prev = vec![0u8; row];
    for chunk in d.chunks(row + 1) {
        if chunk.is_empty() {
            break;
        }
        let kind = chunk[0];
        let mut cur: Vec<u8> = chunk[1..].to_vec();
        cur.resize(row, 0);
        for i in 0..row {
            let a = if i >= bpp { cur[i - bpp] as i32 } else { 0 };
            let b = prev[i] as i32;
            let c = if i >= bpp { prev[i - bpp] as i32 } else { 0 };
            let add = match kind {
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let pp = a + b - c;
                    let (pa, pb, pc) = ((pp - a).abs(), (pp - b).abs(), (pp - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => 0,
            };
            cur[i] = (cur[i] as i32 + add) as u8;
        }
        out.extend_from_slice(&cur);
        prev = cur;
    }
    out
}

/// Where a JPEG ends: just after its EOI marker (or the whole input).
pub fn jpeg_end(d: &[u8]) -> usize {
    // skip white space before the SOI (a line end after the operator)
    let mut i = 0;
    while i + 1 < d.len() && !(d[i] == 0xff && d[i + 1] == 0xd8) {
        if i > 16 {
            return d.len();
        }
        i += 1;
    }
    i += 2;
    // marker segments up to the first scan
    let mut in_scan = false;
    while i + 1 < d.len() {
        if !in_scan {
            if d[i] != 0xff {
                i += 1;
                continue;
            }
            let m = d[i + 1];
            if m == 0xd9 {
                return i + 2;
            }
            if m == 0xff || (0xd0..=0xd7).contains(&m) || m == 0x01 {
                i += if m == 0xff { 1 } else { 2 };
                continue;
            }
            let len = d.get(i + 2..i + 4).map(|s| u16::from_be_bytes([s[0], s[1]]) as usize).unwrap_or(0);
            i += 2 + len;
            if m == 0xda {
                in_scan = true;
            }
        } else if d[i] == 0xff {
            let m = d[i + 1];
            if m == 0x00 || (0xd0..=0xd7).contains(&m) || m == 0xff {
                i += if m == 0xff { 1 } else { 2 };
            } else if m == 0xd9 {
                return i + 2;
            } else {
                in_scan = false;
            }
        } else {
            i += 1;
        }
    }
    d.len()
}

fn sub_file(d: &[u8], p: &Params) -> (Vec<u8>, usize) {
    if p.eod_string.is_empty() {
        if p.eod_count < 0 {
            return (d.to_vec(), d.len());
        }
        let n = (p.eod_count as usize).min(d.len());
        return (d[..n].to_vec(), n);
    }
    let mut seen = 0;
    let mut i = 0;
    let s = &p.eod_string;
    while i + s.len() <= d.len() {
        if &d[i..i + s.len()] == s.as_slice() {
            if seen >= p.eod_count.max(0) {
                return (d[..i].to_vec(), i + s.len());
            }
            seen += 1;
            i += s.len();
        } else {
            i += 1;
        }
    }
    (d.to_vec(), d.len())
}

/// Decrypt eexec data (`r` 55665), hex or binary, dropping the four random bytes.
pub fn eexec_decrypt(d: &[u8]) -> Vec<u8> {
    let first: Vec<u8> = d.iter().copied().filter(|c| !c.is_ascii_whitespace()).take(4).collect();
    let bytes = if first.len() == 4 && first.iter().all(|&c| hex(c).is_some()) { ascii_hex(d).0 } else { d.to_vec() };
    let mut r: u16 = 55665;
    let mut out = Vec::with_capacity(bytes.len());
    for c in bytes {
        out.push(c ^ (r >> 8) as u8);
        r = (c as u16).wrapping_add(r).wrapping_mul(52845).wrapping_add(22719);
    }
    out.drain(..4.min(out.len()));
    out
}

/// Where eexec data ends: at the run of zeros that follows it (64 or more `0`s, line ends
/// between), or the end.
pub fn eexec_end(d: &[u8]) -> usize {
    let mut i = 0;
    while i < d.len() {
        if d[i] == b'0' {
            let mut j = i;
            let mut zeros = 0;
            while j < d.len() && (d[j] == b'0' || d[j] == b'\r' || d[j] == b'\n') {
                if d[j] == b'0' {
                    zeros += 1;
                }
                j += 1;
            }
            if zeros >= 64 {
                return i;
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    d.len()
}

/// A file that reads `bytes`.
pub fn bytes_file(bytes: Vec<u8>) -> FileRef {
    PsFile::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii85_round() {
        let (o, n) = ascii85(b"87cURD]i,\"Ebo80~>rest");
        assert_eq!(o, b"Hello World!");
        assert_eq!(n, 17);
        let (o, _) = ascii85(b"z~>");
        assert_eq!(o, vec![0, 0, 0, 0]);
    }

    #[test]
    fn hex_and_runlength() {
        assert_eq!(ascii_hex(b"48 65 6c6C6f>x"), (b"Hello".to_vec(), 13));
        assert_eq!(ascii_hex(b"4>").0, vec![0x40]);
        let (o, n) = run_length(&[2, b'a', b'b', b'c', 254, b'z', 128, 9]);
        assert_eq!(o, b"abczzz");
        assert_eq!(n, 7);
    }

    #[test]
    fn flate_and_lzw() {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
        e.write_all(b"some data some data some data").unwrap();
        let mut z = e.finish().unwrap();
        let len = z.len();
        z.extend_from_slice(b" trailing");
        let (o, n) = flate(&z);
        assert_eq!(o, b"some data some data some data");
        assert_eq!(n, len);
        let mut enc = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
        let l = enc.encode(b"abababababab").unwrap();
        assert_eq!(lzw(&l, true).0, b"abababababab");
    }

    #[test]
    fn png_predictor() {
        let p = Params { predictor: 12, colors: 1, bpc: 8, columns: 3, ..Default::default() };
        let d = vec![2, 1, 2, 3, 2, 1, 1, 1];
        assert_eq!(predict(d, &p), vec![1, 2, 3, 2, 3, 4]);
    }

    #[test]
    fn eexec() {
        // encrypt "abcd" + "hello" and back
        let plain = b"\0\0\0\0hello";
        let mut r: u16 = 55665;
        let mut enc = vec![];
        for &p in plain {
            let c = p ^ (r >> 8) as u8;
            r = (c as u16).wrapping_add(r).wrapping_mul(52845).wrapping_add(22719);
            enc.push(c);
        }
        assert_eq!(eexec_decrypt(&enc), b"hello");
        let hexed: String = enc.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(eexec_decrypt(hexed.as_bytes()), b"hello");
        let z = format!("abc\n{}\n{}\ncleartomark", "0".repeat(64), "0".repeat(64));
        assert_eq!(eexec_end(z.as_bytes()), 4);
    }
}
