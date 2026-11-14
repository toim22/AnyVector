//! Content stream tokenizer: turns the bytes of a page or form content stream into operators
//! with their operands. Inline images (`BI … ID … EI`) come out as one `BI` operator whose
//! operands are the image dictionary and the raw data.

use lopdf::{Dictionary, Object, StringFormat};

pub struct Op {
    pub op: Vec<u8>,
    pub args: Vec<Object>,
}

fn is_white(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(c: u8) -> bool {
    matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

fn is_regular(c: u8) -> bool {
    !is_white(c) && !is_delim(c)
}

struct Lexer<'a> {
    d: &'a [u8],
    i: usize,
}

enum Tok {
    Obj(Object),
    Keyword(Vec<u8>),
    ArrayOpen,
    ArrayClose,
    DictOpen,
    DictClose,
    /// `{` / `}` (only in PostScript calculator functions, which are not content streams).
    Brace,
}

impl<'a> Lexer<'a> {
    fn skip_white(&mut self) {
        while self.i < self.d.len() {
            let c = self.d[self.i];
            if is_white(c) {
                self.i += 1;
            } else if c == b'%' {
                while self.i < self.d.len() && self.d[self.i] != b'\n' && self.d[self.i] != b'\r' {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Option<Tok> {
        self.skip_white();
        let c = *self.d.get(self.i)?;
        match c {
            b'[' => {
                self.i += 1;
                Some(Tok::ArrayOpen)
            }
            b']' => {
                self.i += 1;
                Some(Tok::ArrayClose)
            }
            b'{' | b'}' => {
                self.i += 1;
                Some(Tok::Brace)
            }
            b'<' if self.d.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                Some(Tok::DictOpen)
            }
            b'>' if self.d.get(self.i + 1) == Some(&b'>') => {
                self.i += 2;
                Some(Tok::DictClose)
            }
            b'>' => {
                self.i += 1;
                Some(Tok::Brace)
            }
            b'<' => Some(Tok::Obj(self.hex_string())),
            b'(' => Some(Tok::Obj(self.literal_string())),
            b')' => {
                self.i += 1;
                Some(Tok::Brace)
            }
            b'/' => {
                self.i += 1;
                Some(Tok::Obj(Object::Name(self.name())))
            }
            _ => {
                let start = self.i;
                while self.i < self.d.len() && is_regular(self.d[self.i]) {
                    self.i += 1;
                }
                if self.i == start {
                    self.i += 1;
                    return Some(Tok::Brace);
                }
                let w = &self.d[start..self.i];
                if let Some(n) = number(w) {
                    return Some(Tok::Obj(n));
                }
                Some(match w {
                    b"true" => Tok::Obj(Object::Boolean(true)),
                    b"false" => Tok::Obj(Object::Boolean(false)),
                    b"null" => Tok::Obj(Object::Null),
                    _ => Tok::Keyword(w.to_vec()),
                })
            }
        }
    }

    fn name(&mut self) -> Vec<u8> {
        let mut out = vec![];
        while self.i < self.d.len() && is_regular(self.d[self.i]) {
            let c = self.d[self.i];
            if c == b'#' {
                if let (Some(h), Some(l)) = (self.d.get(self.i + 1).and_then(|c| hexval(*c)), self.d.get(self.i + 2).and_then(|c| hexval(*c))) {
                    out.push(h * 16 + l);
                    self.i += 3;
                    continue;
                }
            }
            out.push(c);
            self.i += 1;
        }
        out
    }

    fn hex_string(&mut self) -> Object {
        self.i += 1;
        let mut out = vec![];
        let mut hi: Option<u8> = None;
        while self.i < self.d.len() {
            let c = self.d[self.i];
            self.i += 1;
            if c == b'>' {
                break;
            }
            if let Some(v) = hexval(c) {
                match hi.take() {
                    Some(h) => out.push(h * 16 + v),
                    None => hi = Some(v),
                }
            }
        }
        if let Some(h) = hi {
            out.push(h * 16);
        }
        Object::String(out, StringFormat::Hexadecimal)
    }

    fn literal_string(&mut self) -> Object {
        self.i += 1;
        let mut out = vec![];
        let mut depth = 1;
        while self.i < self.d.len() {
            let c = self.d[self.i];
            self.i += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(&e) = self.d.get(self.i) else { break };
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.d.get(self.i) == Some(&b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.d.get(self.i) {
                                    Some(&d) if (b'0'..=b'7').contains(&d) => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        _ => out.push(e),
                    }
                }
                _ => out.push(c),
            }
        }
        Object::String(out, StringFormat::Literal)
    }

    /// The raw data of an inline image, after `ID` and up to the `EI` that ends it.
    fn inline_data(&mut self, len_hint: Option<usize>) -> Vec<u8> {
        // one white-space character follows ID
        if self.i < self.d.len() && is_white(self.d[self.i]) {
            self.i += 1;
        }
        let start = self.i;
        if let Some(n) = len_hint {
            let end = start + n;
            if end <= self.d.len() {
                // trust the size when EI follows it
                let mut j = end;
                while j < self.d.len() && is_white(self.d[j]) {
                    j += 1;
                }
                if self.d[j..].starts_with(b"EI") && self.d.get(j + 2).is_none_or(|c| !is_regular(*c)) {
                    self.i = j + 2;
                    return self.d[start..end].to_vec();
                }
            }
        }
        let mut j = start;
        while j + 2 <= self.d.len() {
            if self.d[j] == b'E' && self.d[j + 1] == b'I' && (j == start || is_white(self.d[j - 1])) && self.d.get(j + 2).is_none_or(|c| is_white(*c)) {
                let mut end = j;
                if end > start && is_white(self.d[end - 1]) {
                    end -= 1;
                }
                self.i = j + 2;
                return self.d[start..end].to_vec();
            }
            j += 1;
        }
        self.i = self.d.len();
        self.d[start..].to_vec()
    }
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn number(w: &[u8]) -> Option<Object> {
    let first = *w.first()?;
    if !(first.is_ascii_digit() || first == b'-' || first == b'+' || first == b'.') {
        return None;
    }
    let s = std::str::from_utf8(w).ok()?;
    if !s.contains('.') {
        if let Ok(v) = s.parse::<i64>() {
            return Some(Object::Integer(v));
        }
    }
    // tolerate "--5", "5-" and the like the way readers do
    let t: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let neg = t.starts_with('-');
    let t = t.trim_start_matches('-');
    let mut seen_dot = false;
    let t: String = t
        .chars()
        .filter(|c| {
            if *c == '.' {
                if seen_dot {
                    return false;
                }
                seen_dot = true;
            }
            *c != '-'
        })
        .collect();
    if t.is_empty() || t == "." {
        return Some(Object::Integer(0));
    }
    let v: f64 = t.parse().ok()?;
    Some(Object::Real((if neg { -v } else { v }) as f32))
}

/// Read one object (with nested arrays and dictionaries) after its first token.
fn object(lx: &mut Lexer, first: Tok, depth: usize) -> Option<Object> {
    if depth > 64 {
        return None;
    }
    match first {
        Tok::Obj(o) => Some(o),
        Tok::ArrayOpen => {
            let mut v = vec![];
            loop {
                let t = lx.next()?;
                match t {
                    Tok::ArrayClose => return Some(Object::Array(v)),
                    Tok::Keyword(_) | Tok::DictClose | Tok::Brace => {}
                    t => {
                        if let Some(o) = object(lx, t, depth + 1) {
                            v.push(o);
                        }
                    }
                }
            }
        }
        Tok::DictOpen => {
            let mut d = Dictionary::new();
            loop {
                let t = lx.next()?;
                match t {
                    Tok::DictClose => return Some(Object::Dictionary(d)),
                    Tok::Obj(Object::Name(k)) => {
                        let t = lx.next()?;
                        if matches!(t, Tok::DictClose) {
                            return Some(Object::Dictionary(d));
                        }
                        if let Some(v) = object(lx, t, depth + 1) {
                            d.set(k, v);
                        }
                    }
                    _ => {}
                }
            }
        }
        Tok::Keyword(_) | Tok::ArrayClose | Tok::DictClose | Tok::Brace => None,
    }
}

/// The operators of a content stream, in order.
pub fn parse(data: &[u8]) -> Vec<Op> {
    let mut lx = Lexer { d: data, i: 0 };
    let mut ops = vec![];
    let mut args: Vec<Object> = vec![];
    while let Some(t) = lx.next() {
        match t {
            Tok::Keyword(k) => {
                if k == b"BI" {
                    // inline image: key/value pairs up to ID
                    let mut d = Dictionary::new();
                    loop {
                        let Some(t) = lx.next() else { break };
                        match t {
                            Tok::Keyword(k) if k == b"ID" => break,
                            Tok::Obj(Object::Name(k)) => {
                                let Some(t) = lx.next() else { break };
                                if let Some(v) = object(&mut lx, t, 1) {
                                    d.set(k, v);
                                }
                            }
                            _ => {}
                        }
                    }
                    let hint = inline_size(&d);
                    let data = lx.inline_data(hint);
                    ops.push(Op { op: b"BI".to_vec(), args: vec![Object::Dictionary(d), Object::String(data, StringFormat::Literal)] });
                    args.clear();
                    continue;
                }
                ops.push(Op { op: k, args: std::mem::take(&mut args) });
            }
            Tok::ArrayClose | Tok::DictClose | Tok::Brace => {}
            t => {
                if let Some(o) = object(&mut lx, t, 0) {
                    args.push(o);
                }
                if args.len() > 4096 {
                    args.clear();
                }
            }
        }
    }
    ops
}

/// The byte size of an unfiltered inline image, when it can be known from its dictionary.
fn inline_size(d: &Dictionary) -> Option<usize> {
    if d.has(b"F") || d.has(b"Filter") {
        return None;
    }
    let get = |a: &[u8], b: &[u8]| d.get(a).or_else(|_| d.get(b)).ok().and_then(|o| o.as_i64().ok());
    let w = get(b"W", b"Width")? as usize;
    let h = get(b"H", b"Height")? as usize;
    let mask = d.get(b"IM").or_else(|_| d.get(b"ImageMask")).ok().and_then(|o| o.as_bool().ok()).unwrap_or(false);
    let bpc = if mask { 1 } else { get(b"BPC", b"BitsPerComponent").unwrap_or(8) as usize };
    let comps = if mask {
        1
    } else {
        match d.get(b"CS").or_else(|_| d.get(b"ColorSpace")).ok() {
            Some(Object::Name(n)) => match n.as_slice() {
                b"G" | b"DeviceGray" | b"I" | b"Indexed" => 1,
                b"RGB" | b"DeviceRGB" => 3,
                b"CMYK" | b"DeviceCMYK" => 4,
                _ => return None,
            },
            Some(Object::Array(a)) if a.first().and_then(|o| o.as_name().ok()).is_some_and(|n| n == b"I" || n == b"Indexed") => 1,
            _ => return None,
        }
    };
    Some((w * comps * bpc).div_ceil(8) * h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operators_and_operands() {
        let ops = parse(b"q 1 0 0 -1 0 792 cm /F1 12 Tf [(A) -120 <0042>] TJ (a\\(b\\)\\101) Tj %c\n<</MCID 3>> BDC BI /W 2 /H 1 /CS /G /BPC 8 ID \x01\x02 EI Q");
        let names: Vec<String> = ops.iter().map(|o| String::from_utf8_lossy(&o.op).into_owned()).collect();
        assert_eq!(names, ["q", "cm", "Tf", "TJ", "Tj", "BDC", "BI", "Q"]);
        assert_eq!(ops[1].args.len(), 6);
        assert_eq!(ops[1].args[3].as_float().unwrap(), -1.0);
        let Object::Array(a) = &ops[3].args[0] else { panic!() };
        assert_eq!(a.len(), 3);
        assert_eq!(a[2].as_str().unwrap(), &[0, 0x42]);
        assert_eq!(ops[4].args[0].as_str().unwrap(), b"a(b)A");
        assert_eq!(ops[6].args[1].as_str().unwrap(), &[1, 2]);
    }
}
