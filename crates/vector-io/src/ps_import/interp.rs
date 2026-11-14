//! The interpreter: stacks, name lookup, execution of procedures and files, the scanner, and
//! what it counts while it runs (operators it doesn't know, errors).

use super::gfx::GState;
use super::obj::*;
use super::output::Builder;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PsErr {
    StackUnderflow,
    TypeCheck,
    RangeCheck,
    Undefined,
    UndefinedResult,
    LimitCheck,
    SyntaxError,
    IoError,
    DictStackUnderflow,
    NoCurrentPoint,
    InvalidFont,
    Unregistered,
}

impl PsErr {
    pub fn name(self) -> &'static str {
        match self {
            PsErr::StackUnderflow => "stackunderflow",
            PsErr::TypeCheck => "typecheck",
            PsErr::RangeCheck => "rangecheck",
            PsErr::Undefined => "undefined",
            PsErr::UndefinedResult => "undefinedresult",
            PsErr::LimitCheck => "limitcheck",
            PsErr::SyntaxError => "syntaxerror",
            PsErr::IoError => "ioerror",
            PsErr::DictStackUnderflow => "dictstackunderflow",
            PsErr::NoCurrentPoint => "nocurrentpoint",
            PsErr::InvalidFont => "invalidfont",
            PsErr::Unregistered => "unregistered",
        }
    }
}

/// How execution leaves the normal path.
pub enum Flow {
    Err(PsErr),
    Exit,
    Stop,
    Quit,
}

impl From<PsErr> for Flow {
    fn from(e: PsErr) -> Flow {
        Flow::Err(e)
    }
}

pub type R<T = ()> = Result<T, Flow>;
pub type OpFn = fn(&mut Interp) -> R;

/// An operator's name and code (a helper so closures turn into `OpFn`).
pub fn op(name: &'static str, f: OpFn) -> (&'static str, OpFn) {
    (name, f)
}

/// Most operators run; past this the program is taken to never end.
const MAX_STEPS: u64 = 400_000_000;
const MAX_TIME: Duration = Duration::from_secs(90);
const MAX_DEPTH: usize = 1500;
/// Errors past this many end the run: the program is not one this interpreter can follow.
const MAX_ERRORS: usize = 20_000;

/// What the run counted.
#[derive(Default)]
pub struct Counts {
    /// Names that were not defined, by name.
    pub unknown: BTreeMap<String, usize>,
    /// Errors by (error, operator).
    pub errors: BTreeMap<(String, String), usize>,
    pub error_total: usize,
    /// Fills with a tiling pattern, drawn in one color.
    pub patterns: usize,
    /// Shadings that could not be turned into a fountain fill.
    pub shadings: usize,
    /// Images left out (data that could not be decoded).
    pub images_skipped: usize,
    /// Strings in composite (Type 0 / CID) fonts left out.
    pub composite_text: usize,
    /// Files the program asked to run or open.
    pub files: usize,
    pub aborted: bool,
    pub ops: u64,
    /// Fonts that are shown in another (name in the file, family used).
    pub fonts: BTreeMap<String, String>,
    /// Pages the program drew (showpage).
    pub pages: usize,
}

pub struct Interp {
    pub names: Vec<Rc<str>>,
    pub name_ids: HashMap<Rc<str>, Name>,
    pub ops: Vec<(&'static str, OpFn)>,
    pub os: Vec<Obj>,
    pub ds: Vec<DictRef>,
    pub systemdict: DictRef,
    pub userdict: DictRef,
    pub globaldict: DictRef,
    /// Illustrator's operators and other names that files use without defining them: looked
    /// up only when the dictionary stack doesn't have a name.
    pub fallback: DictRef,
    pub fontdir: DictRef,
    /// Resources by category (`findresource`).
    pub resources: DictRef,
    /// Files being executed, innermost last.
    pub files: Vec<FileRef>,
    pub gs: GState,
    pub gstack: Vec<GState>,
    /// The graphics state depth of each `save`.
    pub saves: Vec<usize>,
    pub out: Builder,
    pub depth: usize,
    pub stopped: usize,
    pub steps: u64,
    pub started: Instant,
    pub counts: Counts,
    pub packing: bool,
    pub rand: u32,
    pub page_size: Option<(f64, f64)>,
    pub next_fid: u32,
    /// Inside a Type 3 glyph procedure: (width set by setcachedevice / setcharwidth).
    pub glyph_width: Option<(f64, f64)>,
    /// During `charpath` with a Type 3 font: the glyphs' paths, instead of painting them.
    pub charpath_capture: Option<kurbo::BezPath>,
    /// The file gives a bounding box (the page is the box, whatever the program asks).
    pub has_bbox: bool,
}

impl Interp {
    pub fn intern(&mut self, s: &str) -> Name {
        if let Some(&n) = self.name_ids.get(s) {
            return n;
        }
        let rc: Rc<str> = Rc::from(s);
        let n = self.names.len() as Name;
        self.names.push(rc.clone());
        self.name_ids.insert(rc, n);
        n
    }

    pub fn intern_bytes(&mut self, b: &[u8]) -> Name {
        let s: String = b.iter().map(|&c| c as char).collect();
        self.intern(&s)
    }

    pub fn name_str(&self, n: Name) -> Rc<str> {
        self.names[n as usize].clone()
    }

    pub fn lit(&mut self, s: &str) -> Obj {
        Obj::Name(self.intern(s), false)
    }

    pub fn key(&mut self, s: &str) -> Key {
        Key::Name(self.intern(s))
    }

    // ---------------------------------------------------------------- stack helpers

    pub fn push(&mut self, o: Obj) {
        self.os.push(o);
    }

    pub fn pop(&mut self) -> R<Obj> {
        self.os.pop().ok_or(Flow::Err(PsErr::StackUnderflow))
    }

    pub fn need(&self, n: usize) -> R {
        if self.os.len() < n {
            Err(PsErr::StackUnderflow.into())
        } else {
            Ok(())
        }
    }

    pub fn pop_num(&mut self) -> R<f64> {
        match self.os.last() {
            None => Err(PsErr::StackUnderflow.into()),
            Some(o) => match o.num() {
                Some(v) => {
                    self.os.pop();
                    Ok(v)
                }
                None => Err(PsErr::TypeCheck.into()),
            },
        }
    }

    pub fn pop_int(&mut self) -> R<i64> {
        match self.os.last() {
            None => Err(PsErr::StackUnderflow.into()),
            Some(Obj::Int(i)) => {
                let i = *i;
                self.os.pop();
                Ok(i)
            }
            // reals with integer values pass where files are careless
            Some(Obj::Real(r)) if r.fract() == 0.0 && r.abs() < 1e15 => {
                let i = *r as i64;
                self.os.pop();
                Ok(i)
            }
            Some(_) => Err(PsErr::TypeCheck.into()),
        }
    }

    pub fn pop_usize(&mut self) -> R<usize> {
        let i = self.pop_int()?;
        if i < 0 {
            return Err(PsErr::RangeCheck.into());
        }
        Ok(i as usize)
    }

    pub fn pop_bool(&mut self) -> R<bool> {
        match self.pop()? {
            Obj::Bool(b) => Ok(b),
            o => {
                self.os.push(o);
                Err(PsErr::TypeCheck.into())
            }
        }
    }

    pub fn pop_dict(&mut self) -> R<DictRef> {
        match self.pop()? {
            Obj::Dict(d) => Ok(d),
            o => {
                self.os.push(o);
                Err(PsErr::TypeCheck.into())
            }
        }
    }

    pub fn pop_arr(&mut self) -> R<PsArr> {
        match self.pop()? {
            Obj::Array(a, _) => Ok(a),
            o => {
                self.os.push(o);
                Err(PsErr::TypeCheck.into())
            }
        }
    }

    pub fn pop_str(&mut self) -> R<PsStr> {
        match self.pop()? {
            Obj::Str(s, _) => Ok(s),
            o => {
                self.os.push(o);
                Err(PsErr::TypeCheck.into())
            }
        }
    }

    /// A procedure operand (any array; files sometimes pass literal ones).
    pub fn pop_proc(&mut self) -> R<Obj> {
        match self.pop()? {
            o @ (Obj::Array(..) | Obj::Op(_) | Obj::Name(_, true)) => Ok(o),
            o => {
                self.os.push(o);
                Err(PsErr::TypeCheck.into())
            }
        }
    }

    pub fn nums(&mut self, n: usize) -> R<Vec<f64>> {
        self.need(n)?;
        let at = self.os.len() - n;
        let mut v = Vec::with_capacity(n);
        for o in &self.os[at..] {
            v.push(o.num().ok_or(Flow::Err(PsErr::TypeCheck))?);
        }
        self.os.truncate(at);
        Ok(v)
    }

    /// The text of a name or string.
    pub fn text_of(&self, o: &Obj) -> Option<String> {
        match o {
            Obj::Name(n, _) => Some(self.name_str(*n).to_string()),
            Obj::Str(s, _) => Some(s.text()),
            _ => None,
        }
    }

    pub fn to_key(&mut self, o: &Obj) -> R<Key> {
        Ok(match o {
            Obj::Name(n, _) => Key::Name(*n),
            Obj::Str(s, _) => Key::Name(self.intern_bytes(&s.bytes())),
            Obj::Int(i) => Key::Int(*i),
            Obj::Real(r) => {
                if r.fract() == 0.0 && r.abs() < 1e15 {
                    Key::Int(*r as i64)
                } else {
                    Key::Real(r.to_bits())
                }
            }
            Obj::Bool(b) => Key::Bool(*b),
            Obj::Null => return Err(PsErr::TypeCheck.into()),
            Obj::Op(i) => Key::Op(*i),
            Obj::Array(a, _) => Key::Ptr(Rc::as_ptr(&a.buf) as *const u8 as usize + a.start),
            Obj::Dict(d) => Key::Ptr(Rc::as_ptr(d) as *const u8 as usize),
            Obj::File(f, _) => Key::Ptr(Rc::as_ptr(f) as *const u8 as usize),
            Obj::Mark => Key::Mark,
            Obj::Save(s) => Key::Int(-1_000_000 - *s as i64),
            Obj::FontId(i) => Key::Int(-2_000_000 - *i as i64),
        })
    }

    pub fn key_obj(&self, k: &Key) -> Obj {
        match k {
            Key::Name(n) => Obj::Name(*n, false),
            Key::Int(i) => Obj::Int(*i),
            Key::Real(b) => Obj::Real(f64::from_bits(*b)),
            Key::Bool(b) => Obj::Bool(*b),
            Key::Op(i) => Obj::Op(*i),
            Key::Ptr(_) => Obj::Null,
            Key::Mark => Obj::Mark,
        }
    }

    pub fn dget(&mut self, d: &DictRef, k: &str) -> Option<Obj> {
        let k = self.key(k);
        d.borrow().map.get(&k).cloned()
    }

    pub fn dput(&mut self, d: &DictRef, k: &str, v: Obj) {
        let k = self.key(k);
        d.borrow_mut().map.insert(k, v);
    }

    pub fn dnum(&mut self, d: &DictRef, k: &str) -> Option<f64> {
        self.dget(d, k).and_then(|o| o.num())
    }

    pub fn lookup(&self, n: Name) -> Option<Obj> {
        let k = Key::Name(n);
        for d in self.ds.iter().rev() {
            if let Some(v) = d.borrow().map.get(&k) {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn def(&mut self, k: Key, v: Obj) {
        if let Some(d) = self.ds.last() {
            d.borrow_mut().map.insert(k, v);
        }
    }

    pub fn op_index(&self, name: &str) -> Option<u16> {
        self.ops.iter().position(|(n, _)| *n == name).map(|i| i as u16)
    }

    // ---------------------------------------------------------------- execution

    fn step(&mut self) -> R {
        self.steps += 1;
        if self.steps & 0xffff == 0 && (self.steps > MAX_STEPS || self.started.elapsed() > MAX_TIME) {
            self.counts.aborted = true;
            return Err(Flow::Quit);
        }
        Ok(())
    }

    /// Execute an object met in a program or procedure body: names are looked up and run,
    /// procedures are pushed.
    pub fn exec_token(&mut self, o: Obj) -> R {
        match o {
            Obj::Name(n, true) => match self.lookup(n) {
                Some(v) => self.exec_value(v),
                None => self.undefined(n),
            },
            Obj::Op(i) => self.call_op(i),
            o => {
                self.os.push(o);
                Ok(())
            }
        }
    }

    fn undefined(&mut self, n: Name) -> R {
        let k = Key::Name(n);
        let v = self.fallback.borrow().map.get(&k).cloned();
        if let Some(v) = v {
            return self.exec_value(v);
        }
        // Illustrator's procsets, when the file names one it doesn't include
        let s = self.name_str(n);
        if s.starts_with("Adobe_") || s.starts_with("AI") && s.ends_with("_vars") {
            let d = super::ai::stub_procset(self);
            self.fallback.borrow_mut().map.insert(k, Obj::Dict(d.clone()));
            self.os.push(Obj::Dict(d));
            return Ok(());
        }
        Err(Flow::Err(PsErr::Undefined))
    }

    /// Execute an object as `exec` does.
    pub fn exec_value(&mut self, v: Obj) -> R {
        match v {
            Obj::Array(a, true) => self.run_proc(&a),
            Obj::Op(i) => self.call_op(i),
            Obj::Name(n, true) => {
                self.enter()?;
                let r = match self.lookup(n) {
                    Some(v) => self.exec_value(v),
                    None => self.undefined(n),
                };
                self.depth -= 1;
                r
            }
            Obj::Str(s, true) => {
                let f = PsFile::from_bytes(s.bytes());
                self.run_file(f)
            }
            Obj::File(f, true) => self.run_file(f),
            Obj::Null => Ok(()),
            o => {
                self.os.push(o);
                Ok(())
            }
        }
    }

    fn enter(&mut self) -> R {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return Err(PsErr::LimitCheck.into());
        }
        Ok(())
    }

    pub fn call_op(&mut self, i: u16) -> R {
        let f = self.ops[i as usize].1;
        f(self)
    }

    pub fn run_proc(&mut self, a: &PsArr) -> R {
        self.enter()?;
        let mut r = Ok(());
        for i in 0..a.len {
            let Some(o) = a.get(i) else { break };
            if let Err(e) = self.step() {
                r = Err(e);
                break;
            }
            let what = if let Obj::Name(n, true) = &o { Some(Obj::Name(*n, true)) } else if let Obj::Op(_) = &o { Some(o.clone()) } else { None };
            if let Err(f) = self.exec_token(o) {
                match f {
                    Flow::Err(e) => {
                        if let Err(f) = self.error(e, what) {
                            r = Err(f);
                            break;
                        }
                    }
                    f => {
                        r = Err(f);
                        break;
                    }
                }
            }
        }
        self.depth -= 1;
        r
    }

    /// An error: inside `stopped` it stops; otherwise it is counted and the program goes on
    /// with its next object.
    pub fn error(&mut self, e: PsErr, what: Option<Obj>) -> R {
        let name = match &what {
            Some(Obj::Name(n, _)) => self.name_str(*n).to_string(),
            Some(Obj::Op(i)) => self.ops[*i as usize].0.to_string(),
            _ => String::new(),
        };
        if self.stopped > 0 {
            if let Some(d) = self.dget(&self.systemdict.clone(), "$error") {
                if let Obj::Dict(d) = d {
                    let en = self.lit(e.name());
                    self.dput(&d, "errorname", en);
                    let cmd = what.unwrap_or(Obj::Null);
                    self.dput(&d, "command", cmd);
                    self.dput(&d, "newerror", Obj::Bool(true));
                }
            }
            return Err(Flow::Stop);
        }
        if e == PsErr::Undefined && !name.is_empty() {
            *self.counts.unknown.entry(name).or_default() += 1;
        } else {
            *self.counts.errors.entry((e.name().to_string(), name)).or_default() += 1;
        }
        self.counts.error_total += 1;
        if self.counts.error_total > MAX_ERRORS {
            self.counts.aborted = true;
            return Err(Flow::Quit);
        }
        Ok(())
    }

    /// Execute a file to its end (or until it is closed).
    pub fn run_file(&mut self, f: FileRef) -> R {
        self.enter()?;
        self.files.push(f.clone());
        let mut r = Ok(());
        loop {
            if f.borrow().closed {
                break;
            }
            if let Err(e) = self.step() {
                r = Err(e);
                break;
            }
            let tok = match self.next_token(&f) {
                Ok(Some(t)) => t,
                Ok(None) => break,
                Err(e) => {
                    if let Err(fl) = self.error(e, None) {
                        r = Err(fl);
                        break;
                    }
                    continue;
                }
            };
            let what = if let Obj::Name(n, true) = &tok { Some(Obj::Name(*n, true)) } else { None };
            match self.exec_token(tok) {
                Ok(()) => {}
                Err(Flow::Err(e)) => {
                    if let Err(fl) = self.error(e, what) {
                        r = Err(fl);
                        break;
                    }
                }
                // `exit` outside a loop, `stop` outside `stopped`: a printer would end the
                // job; here the rest of the program still runs
                Err(Flow::Exit) => {}
                Err(Flow::Stop) if self.stopped == 0 => {}
                Err(fl) => {
                    r = Err(fl);
                    break;
                }
            }
        }
        self.files.pop();
        self.depth -= 1;
        r
    }

    /// The next token of a file, its procedures read whole.
    pub fn next_token(&mut self, f: &FileRef) -> Result<Option<Obj>, PsErr> {
        super::filters::ensure(f);
        let (data, mut pos) = {
            let b = f.borrow();
            (b.data.clone(), b.pos)
        };
        let r = self.scan(&data, &mut pos, 0);
        f.borrow_mut().pos = pos;
        match r {
            Ok(Scanned::Obj(o)) => Ok(Some(o)),
            Ok(Scanned::Eof) => Ok(None),
            Ok(Scanned::Close) => Err(PsErr::SyntaxError),
            Err(e) => Err(e),
        }
    }

    // ---------------------------------------------------------------- scanner

    pub fn scan(&mut self, d: &[u8], pos: &mut usize, depth: usize) -> Result<Scanned, PsErr> {
        let mut i = *pos;
        // white space and comments
        loop {
            match d.get(i) {
                None => {
                    *pos = i;
                    return Ok(Scanned::Eof);
                }
                Some(b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c | 0x04) => i += 1,
                Some(b'%') => {
                    while i < d.len() && d[i] != b'\n' && d[i] != b'\r' {
                        i += 1;
                    }
                }
                _ => break,
            }
        }
        let c = d[i];
        match c {
            b'(' => {
                let (s, end) = scan_string(d, i + 1);
                *pos = end;
                Ok(Scanned::Obj(Obj::str(s)))
            }
            b'<' => {
                if d.get(i + 1) == Some(&b'<') {
                    *pos = i + 2;
                    Ok(Scanned::Obj(Obj::Name(self.intern("<<"), true)))
                } else if d.get(i + 1) == Some(&b'~') {
                    let (s, end) = super::filters::ascii85(&d[i + 2..]);
                    *pos = i + 2 + end;
                    Ok(Scanned::Obj(Obj::str(s)))
                } else {
                    let (s, end) = super::filters::ascii_hex(&d[i + 1..]);
                    *pos = i + 1 + end;
                    Ok(Scanned::Obj(Obj::str(s)))
                }
            }
            b'>' => {
                if d.get(i + 1) == Some(&b'>') {
                    *pos = i + 2;
                    Ok(Scanned::Obj(Obj::Name(self.intern(">>"), true)))
                } else {
                    *pos = i + 1;
                    Err(PsErr::SyntaxError)
                }
            }
            b'[' | b']' => {
                *pos = i + 1;
                Ok(Scanned::Obj(Obj::Name(self.intern(if c == b'[' { "[" } else { "]" }), true)))
            }
            b'{' => {
                if depth > 200 {
                    *pos = i + 1;
                    return Err(PsErr::LimitCheck);
                }
                let mut items = vec![];
                let mut p = i + 1;
                loop {
                    match self.scan(d, &mut p, depth + 1)? {
                        Scanned::Obj(o) => items.push(o),
                        Scanned::Close => break,
                        Scanned::Eof => {
                            *pos = p;
                            return Err(PsErr::SyntaxError);
                        }
                    }
                }
                *pos = p;
                Ok(Scanned::Obj(Obj::Array(PsArr::new(items), true)))
            }
            b'}' => {
                *pos = i + 1;
                if depth == 0 {
                    Err(PsErr::SyntaxError)
                } else {
                    Ok(Scanned::Close)
                }
            }
            b')' => {
                *pos = i + 1;
                Err(PsErr::SyntaxError)
            }
            b'/' => {
                let immediate = d.get(i + 1) == Some(&b'/');
                let s = if immediate { i + 2 } else { i + 1 };
                let mut e = s;
                while e < d.len() && !is_delim(d[e]) {
                    e += 1;
                }
                *pos = e;
                let n = self.intern_bytes(&d[s..e]);
                if immediate {
                    match self.lookup(n).or_else(|| self.fallback.borrow().map.get(&Key::Name(n)).cloned()) {
                        Some(v) => Ok(Scanned::Obj(v)),
                        None => Err(PsErr::Undefined),
                    }
                } else {
                    Ok(Scanned::Obj(Obj::Name(n, false)))
                }
            }
            _ => {
                let mut e = i;
                while e < d.len() && !is_delim(d[e]) {
                    e += 1;
                }
                if e == i {
                    // a byte that starts nothing (binary tokens are not read)
                    *pos = i + 1;
                    return Err(PsErr::SyntaxError);
                }
                *pos = e;
                let t = &d[i..e];
                if let Some(o) = parse_number(t) {
                    return Ok(Scanned::Obj(o));
                }
                Ok(Scanned::Obj(Obj::Name(self.intern_bytes(t), true)))
            }
        }
    }
}

pub enum Scanned {
    Obj(Obj),
    Close,
    Eof,
}

pub fn is_delim(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c | 0x04 | b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

/// A literal string from just after its `(`: the bytes and where it ends.
pub fn scan_string(d: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut out = vec![];
    let mut nest = 0;
    while i < d.len() {
        let c = d[i];
        i += 1;
        match c {
            b'(' => {
                nest += 1;
                out.push(c);
            }
            b')' => {
                if nest == 0 {
                    return (out, i);
                }
                nest -= 1;
                out.push(c);
            }
            b'\\' => {
                let Some(&e) = d.get(i) else { break };
                i += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'\r' => {
                        if d.get(i) == Some(&b'\n') {
                            i += 1;
                        }
                    }
                    b'\n' => {}
                    b'0'..=b'7' => {
                        let mut v = (e - b'0') as u32;
                        for _ in 0..2 {
                            match d.get(i) {
                                Some(&o @ b'0'..=b'7') => {
                                    v = v * 8 + (o - b'0') as u32;
                                    i += 1;
                                }
                                _ => break,
                            }
                        }
                        out.push((v & 0xff) as u8);
                    }
                    other => out.push(other),
                }
            }
            b'\r' => {
                // an end of line in a string is a newline
                if d.get(i) == Some(&b'\n') {
                    i += 1;
                }
                out.push(b'\n');
            }
            _ => out.push(c),
        }
    }
    (out, i)
}

pub fn parse_number(t: &[u8]) -> Option<Obj> {
    let s = std::str::from_utf8(t).ok()?;
    let first = *t.first()?;
    if !(first.is_ascii_digit() || first == b'+' || first == b'-' || first == b'.') {
        return None;
    }
    if let Some(h) = s.find('#') {
        let base: u32 = s[..h].parse().ok()?;
        if !(2..=36).contains(&base) {
            return None;
        }
        let v = u64::from_str_radix(&s[h + 1..], base).ok()?;
        // radix numbers are unsigned 32-bit patterns
        return Some(if v <= u32::MAX as u64 { Obj::Int(v as u32 as i32 as i64) } else { Obj::Real(v as f64) });
    }
    let digits = s.trim_start_matches(['+', '-']);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        return match s.parse::<i64>() {
            Ok(v) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => Some(Obj::Int(v)),
            _ => s.parse::<f64>().ok().map(Obj::Real),
        };
    }
    // reals: digits with a point and/or an exponent
    let b = digits.as_bytes();
    let mut j = 0;
    let mut mant = 0;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
        mant += 1;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            mant += 1;
        }
    }
    if mant == 0 {
        return None;
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        j += 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let k = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if k == j {
            return None;
        }
    }
    if j != b.len() {
        return None;
    }
    let s2 = if s.ends_with('.') { format!("{s}0") } else { s.to_string() };
    let s2 = if s2.starts_with('.') || s2.starts_with("-.") || s2.starts_with("+.") { s2.replacen('.', "0.", 1) } else { s2 };
    s2.parse::<f64>().ok().map(Obj::Real)
}
