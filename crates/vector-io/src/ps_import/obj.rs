//! PostScript objects: the values on the stacks, arrays and strings that share their storage
//! (as `getinterval` asks), dictionaries and files.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;

/// An interned name.
pub type Name = u32;

/// A dictionary map with a fixed hasher, so that `forall` visits keys in the same order on
/// every run.
pub type Map = HashMap<Key, Obj, BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;

#[derive(Clone)]
pub enum Obj {
    Null,
    Int(i64),
    Real(f64),
    Bool(bool),
    /// A name; `true` when executable.
    Name(Name, bool),
    Str(PsStr, bool),
    /// An array; executable arrays are procedures.
    Array(PsArr, bool),
    Dict(DictRef),
    Op(u16),
    Mark,
    File(FileRef, bool),
    Save(usize),
    /// A font's `FID`.
    FontId(u32),
}

#[derive(Clone)]
pub struct PsStr {
    pub buf: Rc<RefCell<Vec<u8>>>,
    pub start: usize,
    pub len: usize,
}

impl PsStr {
    pub fn new(bytes: Vec<u8>) -> PsStr {
        let len = bytes.len();
        PsStr { buf: Rc::new(RefCell::new(bytes)), start: 0, len }
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.buf.borrow()[self.start..self.start + self.len].to_vec()
    }
    pub fn get(&self, i: usize) -> Option<u8> {
        (i < self.len).then(|| self.buf.borrow()[self.start + i])
    }
    pub fn put(&self, i: usize, v: u8) -> bool {
        if i < self.len {
            self.buf.borrow_mut()[self.start + i] = v;
            true
        } else {
            false
        }
    }
    pub fn sub(&self, start: usize, len: usize) -> Option<PsStr> {
        (start + len <= self.len).then(|| PsStr { buf: self.buf.clone(), start: self.start + start, len })
    }
    pub fn text(&self) -> String {
        self.buf.borrow()[self.start..self.start + self.len].iter().map(|&b| b as char).collect()
    }
}

#[derive(Clone)]
pub struct PsArr {
    pub buf: Rc<RefCell<Vec<Obj>>>,
    pub start: usize,
    pub len: usize,
}

impl PsArr {
    pub fn new(items: Vec<Obj>) -> PsArr {
        let len = items.len();
        PsArr { buf: Rc::new(RefCell::new(items)), start: 0, len }
    }
    pub fn get(&self, i: usize) -> Option<Obj> {
        (i < self.len).then(|| self.buf.borrow()[self.start + i].clone())
    }
    pub fn put(&self, i: usize, v: Obj) -> bool {
        if i < self.len {
            self.buf.borrow_mut()[self.start + i] = v;
            true
        } else {
            false
        }
    }
    pub fn items(&self) -> Vec<Obj> {
        self.buf.borrow()[self.start..self.start + self.len].to_vec()
    }
    pub fn sub(&self, start: usize, len: usize) -> Option<PsArr> {
        (start + len <= self.len).then(|| PsArr { buf: self.buf.clone(), start: self.start + start, len })
    }
    pub fn same(&self, o: &PsArr) -> bool {
        Rc::ptr_eq(&self.buf, &o.buf) && self.start == o.start && self.len == o.len
    }
}

pub type DictRef = Rc<RefCell<Dict>>;

#[derive(Default)]
pub struct Dict {
    pub map: Map,
    pub cap: usize,
}

pub fn new_dict(cap: usize) -> DictRef {
    Rc::new(RefCell::new(Dict { map: Map::default(), cap }))
}

/// A dictionary key. Strings are turned into names and integral reals into integers, so the
/// keys compare the way PostScript compares them.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Name(Name),
    Int(i64),
    Real(u64),
    Bool(bool),
    Op(u16),
    /// Composite objects, by identity.
    Ptr(usize),
    Mark,
}

pub type FileRef = Rc<RefCell<PsFile>>;

/// A file: bytes being read (the program, or data decoded by a filter).
pub struct PsFile {
    /// The bytes once known (a filter decodes everything at its first read).
    pub data: Rc<[u8]>,
    pub pos: usize,
    pub closed: bool,
    /// A filter not yet decoded: its name, parameters and source.
    pub pending: Option<Filter>,
    /// The bytes are a JPEG image (a DCTDecode filter, which hands images the encoded data).
    pub dct: bool,
}

pub struct Filter {
    pub name: String,
    pub params: super::filters::Params,
    pub source: FilterSource,
}

pub enum FilterSource {
    File(FileRef),
    Bytes(Vec<u8>),
}

impl PsFile {
    pub fn from_bytes(data: Vec<u8>) -> FileRef {
        Rc::new(RefCell::new(PsFile { data: data.into(), pos: 0, closed: false, pending: None, dct: false }))
    }
    pub fn filter(f: Filter) -> FileRef {
        Rc::new(RefCell::new(PsFile { data: Rc::from(Vec::new()), pos: 0, closed: false, pending: Some(f), dct: false }))
    }
}

impl Obj {
    pub fn type_name(&self) -> &'static str {
        match self {
            Obj::Null => "nulltype",
            Obj::Int(_) => "integertype",
            Obj::Real(_) => "realtype",
            Obj::Bool(_) => "booleantype",
            Obj::Name(..) => "nametype",
            Obj::Str(..) => "stringtype",
            Obj::Array(..) => "arraytype",
            Obj::Dict(_) => "dicttype",
            Obj::Op(_) => "operatortype",
            Obj::Mark => "marktype",
            Obj::File(..) => "filetype",
            Obj::Save(_) => "savetype",
            Obj::FontId(_) => "fonttype",
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Obj::Int(i) => Some(*i as f64),
            Obj::Real(r) => Some(*r),
            _ => None,
        }
    }

    pub fn is_exec(&self) -> bool {
        match self {
            Obj::Name(_, x) | Obj::Str(_, x) | Obj::Array(_, x) | Obj::File(_, x) => *x,
            Obj::Op(_) => true,
            _ => false,
        }
    }

    /// The same object, executable or literal.
    pub fn with_exec(self, exec: bool) -> Obj {
        match self {
            Obj::Name(n, _) => Obj::Name(n, exec),
            Obj::Str(s, _) => Obj::Str(s, exec),
            Obj::Array(a, _) => Obj::Array(a, exec),
            Obj::File(f, _) => Obj::File(f, exec),
            o => o,
        }
    }

    /// A number as an integer when it fits, else a real (the result of integer arithmetic).
    pub fn int_or_real(v: i128) -> Obj {
        if v >= i32::MIN as i128 && v <= i32::MAX as i128 {
            Obj::Int(v as i64)
        } else {
            Obj::Real(v as f64)
        }
    }

    pub fn str(bytes: Vec<u8>) -> Obj {
        Obj::Str(PsStr::new(bytes), false)
    }

    pub fn arr(items: Vec<Obj>) -> Obj {
        Obj::Array(PsArr::new(items), false)
    }

    pub fn proc_(items: Vec<Obj>) -> Obj {
        Obj::Array(PsArr::new(items), true)
    }
}
