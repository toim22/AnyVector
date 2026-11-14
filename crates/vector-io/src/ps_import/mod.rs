//! Import of PostScript: EPS (with or without the DOS binary header and its TIFF/WMF preview),
//! PS, and PostScript-based Illustrator files (versions 3 to 8).
//!
//! The program runs in a PostScript interpreter of its own ([`interp`]); what it paints
//! becomes AnyVector objects ([`output`]). Operators it doesn't know are skipped and counted.
//! When the program paints nothing it could read, the EPS preview image is imported instead.

mod ai;
mod encodings;
mod filters;
mod gfx;
mod images;
mod interp;
mod obj;
mod ops_base;
mod output;
mod shading;
mod text;

#[cfg(test)]
mod tests;

use crate::{IoError, Result};
use interp::{Counts, Interp};
use kurbo::Affine;
use obj::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use vector_core::document::{Document, ImageData};
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};

/// The parts of an EPS or PS file.
#[derive(Default)]
pub struct Parts {
    pub ps: Vec<u8>,
    pub tiff: Option<Vec<u8>>,
    pub wmf: Option<Vec<u8>>,
    /// %%HiResBoundingBox, else %%BoundingBox: llx lly urx ury.
    pub bbox: Option<[f64; 4]>,
    pub eps: bool,
    /// Page size from %%DocumentMedia.
    pub media: Option<(f64, f64)>,
}

/// Whether bytes are PostScript (or a DOS EPS).
pub fn is_postscript(b: &[u8]) -> bool {
    b.starts_with(&[0xC5, 0xD0, 0xD3, 0xC6]) || find(&b[..b.len().min(2048)], b"%!").is_some()
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn u32_at(b: &[u8], p: usize) -> usize {
    b.get(p..p + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).unwrap_or(0)
}

/// Split a file into its PostScript and previews, and read its DSC comments.
pub fn parts(b: &[u8]) -> Result<Parts> {
    let mut p = Parts::default();
    if b.starts_with(b"%PDF") {
        return Err(IoError::Unsupported("PDF-based Illustrator file (save it as Illustrator 8 or EPS, or import it as PDF)".into()));
    }
    if b.starts_with(&[0xC5, 0xD0, 0xD3, 0xC6]) {
        let slice = |start: usize, len: usize| -> Option<Vec<u8>> { (len > 0).then(|| b.get(start..start.saturating_add(len).min(b.len())).map(|s| s.to_vec())).flatten() };
        p.ps = slice(u32_at(b, 4), u32_at(b, 8)).unwrap_or_default();
        p.wmf = slice(u32_at(b, 12), u32_at(b, 16));
        p.tiff = slice(u32_at(b, 20), u32_at(b, 24));
    } else {
        // skip anything before %! (a PJL header, a ^D)
        let start = find(&b[..b.len().min(4096)], b"%!").unwrap_or(0);
        p.ps = b[start..].to_vec();
    }
    let ps = &p.ps;
    let first_line_end = ps.iter().position(|&c| c == b'\n' || c == b'\r').unwrap_or(ps.len());
    p.eps = find(&ps[..first_line_end], b"EPSF").is_some();
    // header comments, and the trailer for "(atend)"
    let head = &ps[..ps.len().min(64 * 1024)];
    let tail = &ps[ps.len().saturating_sub(64 * 1024)..];
    let bbox_in = |region: &[u8], key: &[u8]| -> Option<[f64; 4]> {
        let mut at = 0;
        while let Some(k) = find(&region[at..], key) {
            let s = at + k + key.len();
            let e = region[s..].iter().position(|&c| c == b'\n' || c == b'\r').map(|x| s + x).unwrap_or(region.len());
            let line = String::from_utf8_lossy(&region[s..e]);
            let v: Vec<f64> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            if v.len() == 4 && v[2] > v[0] && v[3] > v[1] {
                return Some([v[0], v[1], v[2], v[3]]);
            }
            at = e;
        }
        None
    };
    p.bbox = bbox_in(head, b"%%HiResBoundingBox:")
        .or_else(|| bbox_in(head, b"%%BoundingBox:"))
        .or_else(|| bbox_in(tail, b"%%HiResBoundingBox:"))
        .or_else(|| bbox_in(tail, b"%%BoundingBox:"));
    if let Some(k) = find(head, b"%%DocumentMedia:") {
        let s = k + 16;
        let e = head[s..].iter().position(|&c| c == b'\n' || c == b'\r').map(|x| s + x).unwrap_or(head.len());
        let line = String::from_utf8_lossy(&head[s..e]);
        let v: Vec<f64> = line.split_whitespace().skip(1).take(2).filter_map(|t| t.parse().ok()).collect();
        if v.len() == 2 && v[0] > 0.0 && v[1] > 0.0 {
            p.media = Some((v[0], v[1]));
        }
    }
    Ok(p)
}

/// What a run of the interpreter gave.
struct Run {
    doc: Document,
    pages: Vec<Vec<Object>>,
    size: (f64, f64),
    counts: Counts,
    painted: usize,
}

impl Interp {
    fn new(to_doc: Affine) -> Interp {
        let mut ops = ops_base::ops();
        ops.extend(gfx::ops());
        ops.extend(text::ops());
        ops.extend(shading::ops());
        ops.extend(images::ops());
        let systemdict = new_dict(512);
        let mut i = Interp {
            names: vec![],
            name_ids: HashMap::new(),
            ops,
            os: vec![],
            ds: vec![],
            systemdict: systemdict.clone(),
            userdict: new_dict(200),
            globaldict: new_dict(64),
            fallback: new_dict(200),
            fontdir: new_dict(64),
            resources: new_dict(16),
            files: vec![],
            gs: gfx::GState::new(),
            gstack: vec![],
            saves: vec![],
            out: output::Builder::new(to_doc),
            depth: 0,
            stopped: 0,
            steps: 0,
            started: Instant::now(),
            counts: Counts::default(),
            packing: false,
            rand: 1,
            page_size: None,
            next_fid: 0,
            glyph_width: None,
            charpath_capture: None,
            has_bbox: false,
        };
        for k in 0..i.ops.len() {
            let n = i.ops[k].0;
            i.dput(&systemdict, n, Obj::Op(k as u16));
        }
        let (ud, gd, fd) = (i.userdict.clone(), i.globaldict.clone(), i.fontdir.clone());
        i.dput(&systemdict, "systemdict", Obj::Dict(systemdict.clone()));
        i.dput(&systemdict, "userdict", Obj::Dict(ud.clone()));
        i.dput(&systemdict, "globaldict", Obj::Dict(gd.clone()));
        i.dput(&systemdict, "FontDirectory", Obj::Dict(fd.clone()));
        i.dput(&systemdict, "GlobalFontDirectory", Obj::Dict(fd.clone()));
        i.dput(&systemdict, "SharedFontDirectory", Obj::Dict(fd));
        for name in ["statusdict", "serverdict", "errordict", "$error", "AVInternal"] {
            let d = new_dict(32);
            i.dput(&systemdict, name, Obj::Dict(d));
        }
        if let Some(Obj::Dict(sd)) = i.dget(&systemdict, "statusdict") {
            i.dput(&sd, "product", Obj::str(b"AnyVector PostScript Import".to_vec()));
            i.dput(&sd, "jobname", Obj::str(b"import".to_vec()));
            let pop = Obj::Op(i.op_index("pop").unwrap());
            i.dput(&sd, "setjobtimeout", Obj::proc_(vec![pop.clone()]));
            i.dput(&sd, "waittimeout", Obj::Int(300));
            i.dput(&sd, "manualfeed", Obj::Bool(false));
            i.dput(&sd, "processcolors", Obj::proc_(vec![Obj::Int(4)]));
        }
        let encs: [(&str, Vec<&str>); 2] = [("StandardEncoding", encodings::standard()), ("ISOLatin1Encoding", encodings::iso_latin1())];
        for (name, enc) in encs {
            let arr: Vec<Obj> = enc.iter().map(|g| i.lit(g)).collect();
            i.dput(&systemdict, name, Obj::arr(arr));
        }
        // a symbol font's codes stay as they are
        let sym: Vec<Obj> = (0..256).map(|_| i.lit(".notdef")).collect();
        i.dput(&systemdict, "AVSymbolEncoding", Obj::arr(sym));
        i.dput(&ud, "#copies", Obj::Int(1));
        i.ds = vec![systemdict.clone(), gd, ud];
        // Illustrator's operators, defined in the fallback dictionary
        let fb = i.fallback.clone();
        i.ds.push(fb);
        let _ = i.run_file(filters::bytes_file(ai::PROCSET.as_bytes().to_vec()));
        i.ds.pop();
        i.os.clear();
        i.counts = Counts::default();
        // a round dot, for currentscreen
        let dot = {
            let names = ["dup", "mul", "exch", "dup", "mul", "add", "1", "exch", "sub"];
            let items = names
                .iter()
                .map(|n| match n.parse::<i64>() {
                    Ok(v) => Obj::Int(v),
                    Err(_) => Obj::Op(i.op_index(n).unwrap()),
                })
                .collect();
            Obj::proc_(items)
        };
        i.dput(&systemdict, "AVDot", dot);
        i.steps = 0;
        i
    }

    /// The page size a program asks for, when the file doesn't give a bounding box.
    pub fn set_page_size(&mut self, w: f64, h: f64) {
        if self.has_bbox || self.out.painted > 0 {
            return;
        }
        self.page_size = Some((w, h));
        self.out.to_doc = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, h]);
    }
}

fn run(parts: &Parts) -> Run {
    let (size, to_doc) = match parts.bbox {
        Some([x0, y0, x1, y1]) => ((x1 - x0, y1 - y0), Affine::new([1.0, 0.0, 0.0, -1.0, -x0, y1])),
        None => {
            let (w, h) = parts.media.unwrap_or((612.0, 792.0));
            ((w, h), Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, h]))
        }
    };
    let mut i = Interp::new(to_doc);
    i.has_bbox = parts.bbox.is_some();
    i.page_size = Some(size);
    let f = filters::bytes_file(parts.ps.clone());
    let _ = i.run_file(f);
    if i.out.has_content() {
        i.out.finish_page();
    }
    let size = i.page_size.unwrap_or(size);
    let painted = i.out.painted;
    let pages = std::mem::take(&mut i.out.pages);
    let mut doc = std::mem::replace(&mut i.out.doc, Document::new(1.0, 1.0));
    for s in std::mem::take(&mut i.out.spots) {
        if !doc.palette.iter().any(|p| p.name == s.name) {
            doc.palette.push(s);
        }
    }
    i.counts.ops = i.steps;
    Run { doc, pages, size, counts: std::mem::take(&mut i.counts), painted }
}

/// Run the interpreter on a thread with room for deeply nested procedures.
fn run_threaded(parts: Parts) -> Result<(Run, Parts)> {
    std::thread::Builder::new()
        .name("ps-import".into())
        .stack_size(256 << 20)
        .spawn(move || {
            let r = run(&parts);
            (r, parts)
        })
        .map_err(IoError::Io)?
        .join()
        .map_err(|_| IoError::Format("the PostScript interpreter stopped unexpectedly".into()))
}

/// What the import left out or approximated, one line each (keys of the translation files).
fn notes(c: &Counts, pages: usize, import: bool) -> Vec<String> {
    let mut out = vec![];
    if !c.unknown.is_empty() {
        let total: usize = c.unknown.values().sum();
        let mut names: Vec<(&String, &usize)> = c.unknown.iter().collect();
        names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let list: Vec<String> = names.iter().take(8).map(|(n, _)| n.to_string()).collect();
        let mut list = list.join(", ");
        if names.len() > 8 {
            list.push_str(", …");
        }
        out.push(format!("Left out: {} use(s) of PostScript operators AnyVector doesn't run ({})", total, list));
    }
    if !c.errors.is_empty() {
        let total: usize = c.errors.values().sum();
        let mut errs: Vec<(&(String, String), &usize)> = c.errors.iter().collect();
        errs.sort_by(|a, b| b.1.cmp(a.1));
        let list: Vec<String> = errs.iter().take(5).map(|((e, o), _)| if o.is_empty() { e.clone() } else { format!("{e} in {o}") }).collect();
        out.push(format!("Skipped {} PostScript error(s) ({})", total, list.join(", ")));
    }
    if c.patterns > 0 {
        out.push(format!("Pattern fills are shown as plain colors ({} fill(s))", c.patterns));
    }
    if c.shadings > 0 {
        out.push(format!("Shadings that aren't linear or radial are shown as plain colors ({})", c.shadings));
    }
    if c.images_skipped > 0 {
        out.push(format!("Left out: {} image(s) whose data couldn't be read", c.images_skipped));
    }
    if c.composite_text > 0 {
        out.push(format!("Left out: {} text string(s) in composite (CID) fonts", c.composite_text));
    }
    for (missing, used) in &c.fonts {
        out.push(format!("The font \"{}\" isn't installed, so its text is shown in \"{}\"", missing, used));
    }
    if c.files > 0 {
        out.push(format!("Left out: {} other file(s) the PostScript asked to read", c.files));
    }
    if c.aborted {
        out.push("The PostScript program was stopped before its end; what it drew until then was imported".to_string());
    }
    if import && pages > 1 {
        out.push(format!("Only the first of {} pages was imported", pages));
    }
    out
}

/// The preview image as a document page filling the bounding box.
fn preview_doc(parts: &Parts) -> Option<Document> {
    let tiff = parts.tiff.as_ref()?;
    let img = image::load_from_memory(tiff).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let (pw, ph) = match parts.bbox {
        Some([x0, y0, x1, y1]) => (x1 - x0, y1 - y0),
        None => (w as f64 * 72.0 / 96.0, h as f64 * 72.0 / 96.0),
    };
    let mut doc = Document::new(pw.max(1.0), ph.max(1.0));
    let data = ImageData { width: w, height: h, rgba: Arc::new(img.into_raw()), encoded: Arc::new(vec![]), format: "png".into(), name: "EPS preview".into() };
    let iid = doc.add_image(data);
    let id = doc.new_id();
    let st = Style { fill: Fill::None, outline: Outline::none(), ..Style::default() };
    let o = Object::new(id, ObjKind::Image(ImageObj { image: iid, w: pw, h: ph, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), Affine::IDENTITY, st);
    doc.pages[0].layers[0].objects.push(o);
    Some(doc)
}

const PREVIEW_NOTE: &str = "The PostScript couldn't be read, so the file's preview image was imported instead";

/// Read a PostScript file into a document (one page per page it shows), with notes on what
/// was left out.
pub fn load_with_notes(bytes: &[u8]) -> Result<(Document, Vec<String>)> {
    load(bytes, false)
}

fn load(bytes: &[u8], import: bool) -> Result<(Document, Vec<String>)> {
    let parts = parts(bytes)?;
    if parts.ps.is_empty() {
        if let Some(doc) = preview_doc(&parts) {
            return Ok((doc, vec![PREVIEW_NOTE.to_string()]));
        }
        return Err(IoError::Format("the file holds no PostScript".into()));
    }
    let (r, parts) = run_threaded(parts)?;
    let nothing = r.painted == 0 || r.pages.iter().all(|p| p.is_empty());
    if nothing {
        if let Some(doc) = preview_doc(&parts) {
            let mut n = vec![PREVIEW_NOTE.to_string()];
            n.extend(notes(&r.counts, 0, import).into_iter().filter(|l| l.starts_with("Left out: ") && l.contains("operators")));
            return Ok((doc, n));
        }
        if parts.wmf.is_some() {
            return Err(IoError::Format("the PostScript couldn't be read, and the file's preview is a Windows Metafile, which AnyVector can't show".into()));
        }
        return Err(IoError::Format("the PostScript draws nothing AnyVector can read".into()));
    }
    let mut doc = r.doc;
    let (w, h) = (r.size.0.max(1.0), r.size.1.max(1.0));
    let npages = r.pages.iter().filter(|p| !p.is_empty()).count();
    let mut first = true;
    for objs in r.pages.into_iter().filter(|p| !p.is_empty()) {
        let idx = if first {
            doc.pages[0].width = w;
            doc.pages[0].height = h;
            0
        } else {
            let n = doc.pages.len();
            doc.add_page(n, w, h)
        };
        first = false;
        doc.pages[idx].layers[0].objects = objs;
        if import {
            break;
        }
    }
    let n = notes(&r.counts, npages, import);
    Ok((doc, n))
}

/// Import a PostScript file into a document: its objects (the first page's, grouped) and
/// notes. Spot colors join the document palette.
pub fn import(doc: &mut Document, bytes: &[u8]) -> Result<(Vec<Object>, Vec<String>)> {
    let (other, notes) = load(bytes, true)?;
    for s in &other.palette {
        if !doc.palette.iter().any(|p| p.name == s.name) {
            doc.palette.push(s.clone());
        }
    }
    let mut objs = crate::merge_document(doc, other);
    if objs.len() > 1 {
        let id = doc.new_id();
        let g = Object::new(id, ObjKind::Group(std::mem::take(&mut objs)), Affine::IDENTITY, Style::default());
        objs.push(g);
    }
    Ok((objs, notes))
}
