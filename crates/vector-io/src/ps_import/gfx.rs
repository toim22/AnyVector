//! The graphics state and its operators: matrices, paths, painting, clipping, line attributes
//! and colors.

use super::interp::{op, Flow, Interp, OpFn, PsErr, R};
use super::obj::*;
use super::output::ClipNode;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};
use std::rc::Rc;
use vector_core::color::Color;
use vector_core::style::{Fill, LineCap, LineJoin, Outline, Style};

#[derive(Clone)]
pub enum CSpace {
    Gray,
    Rgb,
    Cmyk,
    Lab,
    Sep { name: String, alt: Box<CSpace>, tint: Obj },
    DevN { names: Vec<String>, alt: Box<CSpace>, tint: Obj },
    Indexed { base: Box<CSpace>, hival: usize, lookup: Obj },
    Pattern(Option<Box<CSpace>>),
}

impl CSpace {
    pub fn n(&self) -> usize {
        match self {
            CSpace::Gray | CSpace::Sep { .. } | CSpace::Indexed { .. } => 1,
            CSpace::Rgb | CSpace::Lab => 3,
            CSpace::Cmyk => 4,
            CSpace::DevN { names, .. } => names.len().max(1),
            CSpace::Pattern(_) => 0,
        }
    }

    pub fn initial(&self) -> Vec<f64> {
        match self {
            CSpace::Gray | CSpace::Indexed { .. } => vec![0.0],
            CSpace::Rgb | CSpace::Lab => vec![0.0; 3],
            CSpace::Cmyk => vec![0.0, 0.0, 0.0, 1.0],
            CSpace::Sep { .. } => vec![1.0],
            CSpace::DevN { names, .. } => vec![1.0; names.len().max(1)],
            CSpace::Pattern(_) => vec![],
        }
    }
}

#[derive(Clone)]
pub struct PsColor {
    pub space: CSpace,
    /// The color space as the program set it (for `currentcolorspace`).
    pub space_obj: Option<Obj>,
    pub comps: Vec<f64>,
    pub pattern: Option<DictRef>,
}

impl PsColor {
    pub fn gray(g: f64) -> PsColor {
        PsColor { space: CSpace::Gray, space_obj: None, comps: vec![g], pattern: None }
    }
}

#[derive(Clone)]
pub struct GState {
    pub ctm: Affine,
    pub path: BezPath,
    pub cur: Option<Point>,
    pub start: Option<Point>,
    pub clip: Option<Rc<ClipNode>>,
    pub clip_saves: Vec<Option<Rc<ClipNode>>>,
    pub color: PsColor,
    pub lw: f64,
    pub cap: i64,
    pub join: i64,
    pub miter: f64,
    pub dash: Vec<f64>,
    pub dash_off: f64,
    pub font: Option<DictRef>,
    pub overprint: bool,
    pub null_device: bool,
    /// The path is the outline of a stroke (`strokepath`): filling it strokes the original.
    pub stroked: Option<(BezPath, Outline)>,
    pub flat: f64,
    pub stroke_adjust: bool,
    pub smoothness: f64,
    /// What `current…` operators give back for settings that change nothing here.
    pub extra: Vec<(&'static str, Obj)>,
}

impl GState {
    pub fn new() -> GState {
        GState {
            ctm: Affine::IDENTITY,
            path: BezPath::new(),
            cur: None,
            start: None,
            clip: None,
            clip_saves: vec![],
            color: PsColor::gray(0.0),
            lw: 1.0,
            cap: 0,
            join: 0,
            miter: 10.0,
            dash: vec![],
            dash_off: 0.0,
            font: None,
            overprint: false,
            null_device: false,
            stroked: None,
            flat: 1.0,
            stroke_adjust: false,
            smoothness: 0.02,
            extra: vec![],
        }
    }

    pub fn newpath(&mut self) {
        self.path = BezPath::new();
        self.cur = None;
        self.start = None;
        self.stroked = None;
    }
}

pub fn ops() -> Vec<(&'static str, OpFn)> {
    vec![
        op("gsave", |i| {
            i.gsave();
            Ok(())
        }),
        op("grestore", |i| {
            i.grestore();
            Ok(())
        }),
        op("grestoreall", |i| {
            let floor = i.saves.last().copied().unwrap_or(0);
            while i.gstack.len() > floor {
                i.grestore();
            }
            Ok(())
        }),
        op("initgraphics", |i| {
            let font = i.gs.font.clone();
            i.gs = GState::new();
            i.gs.font = font;
            Ok(())
        }),
        op("clipsave", |i| {
            let c = i.gs.clip.clone();
            i.gs.clip_saves.push(c);
            Ok(())
        }),
        op("cliprestore", |i| {
            if let Some(c) = i.gs.clip_saves.pop() {
                i.gs.clip = c;
            }
            Ok(())
        }),
        // ------------------------------------------------ line attributes
        op("setlinewidth", |i| {
            i.gs.lw = i.pop_num()?.abs();
            Ok(())
        }),
        op("currentlinewidth", |i| {
            i.push(Obj::Real(i.gs.lw));
            Ok(())
        }),
        op("setlinecap", |i| {
            i.gs.cap = i.pop_int()?;
            Ok(())
        }),
        op("currentlinecap", |i| {
            i.push(Obj::Int(i.gs.cap));
            Ok(())
        }),
        op("setlinejoin", |i| {
            i.gs.join = i.pop_int()?;
            Ok(())
        }),
        op("currentlinejoin", |i| {
            i.push(Obj::Int(i.gs.join));
            Ok(())
        }),
        op("setmiterlimit", |i| {
            i.gs.miter = i.pop_num()?.max(1.0);
            Ok(())
        }),
        op("currentmiterlimit", |i| {
            i.push(Obj::Real(i.gs.miter));
            Ok(())
        }),
        op("setdash", |i| {
            let off = i.pop_num()?;
            let a = i.pop_arr()?;
            i.gs.dash = a.items().iter().filter_map(|o| o.num()).map(f64::abs).collect();
            if i.gs.dash.iter().all(|d| *d == 0.0) {
                i.gs.dash.clear();
            }
            i.gs.dash_off = off;
            Ok(())
        }),
        op("currentdash", |i| {
            let a = Obj::arr(i.gs.dash.iter().map(|d| Obj::Real(*d)).collect());
            i.push(a);
            i.push(Obj::Real(i.gs.dash_off));
            Ok(())
        }),
        op("setflat", |i| {
            i.gs.flat = i.pop_num()?;
            Ok(())
        }),
        op("currentflat", |i| {
            i.push(Obj::Real(i.gs.flat));
            Ok(())
        }),
        op("setstrokeadjust", |i| {
            i.gs.stroke_adjust = i.pop_bool()?;
            Ok(())
        }),
        op("currentstrokeadjust", |i| {
            i.push(Obj::Bool(i.gs.stroke_adjust));
            Ok(())
        }),
        op("setsmoothness", |i| {
            i.gs.smoothness = i.pop_num()?;
            Ok(())
        }),
        op("currentsmoothness", |i| {
            i.push(Obj::Real(i.gs.smoothness));
            Ok(())
        }),
        op("setoverprint", |i| {
            i.gs.overprint = i.pop_bool()?;
            Ok(())
        }),
        op("currentoverprint", |i| {
            i.push(Obj::Bool(i.gs.overprint));
            Ok(())
        }),
        // ------------------------------------------------ device settings that change nothing
        op("setscreen", |i| set_extra(i, "screen", 3)),
        op("currentscreen", |i| {
            match get_extra(i, "screen") {
                Some(Obj::Array(a, _)) => {
                    for o in a.items() {
                        i.push(o);
                    }
                }
                _ => {
                    i.push(Obj::Real(60.0));
                    i.push(Obj::Real(45.0));
                    let p = i.dget(&i.systemdict.clone(), "AVDot").unwrap_or(Obj::proc_(vec![]));
                    i.push(p);
                }
            }
            Ok(())
        }),
        op("setcolorscreen", |i| set_extra(i, "colorscreen", 12)),
        op("currentcolorscreen", |i| {
            for _ in 0..4 {
                i.push(Obj::Real(60.0));
                i.push(Obj::Real(45.0));
                i.push(Obj::proc_(vec![]));
            }
            Ok(())
        }),
        op("sethalftone", |i| set_extra(i, "halftone", 1)),
        op("currenthalftone", |i| {
            let d = get_extra(i, "halftone").unwrap_or_else(|| {
                let d = new_dict(8);
                Obj::Dict(d)
            });
            if let Obj::Dict(d) = &d {
                i.dput(d, "HalftoneType", Obj::Int(1));
                if i.dget(d, "Frequency").is_none() {
                    i.dput(d, "Frequency", Obj::Real(60.0));
                    i.dput(d, "Angle", Obj::Real(45.0));
                    i.dput(d, "SpotFunction", Obj::proc_(vec![]));
                }
            }
            i.push(d);
            Ok(())
        }),
        op("settransfer", |i| set_extra(i, "transfer", 1)),
        op("currenttransfer", |i| {
            let p = get_extra(i, "transfer").unwrap_or(Obj::proc_(vec![]));
            i.push(p);
            Ok(())
        }),
        op("setcolortransfer", |i| set_extra(i, "colortransfer", 4)),
        op("currentcolortransfer", |i| {
            for _ in 0..4 {
                i.push(Obj::proc_(vec![]));
            }
            Ok(())
        }),
        op("setblackgeneration", |i| set_extra(i, "bg", 1)),
        op("currentblackgeneration", |i| {
            let p = get_extra(i, "bg").unwrap_or(Obj::proc_(vec![]));
            i.push(p);
            Ok(())
        }),
        op("setundercolorremoval", |i| set_extra(i, "ucr", 1)),
        op("currentundercolorremoval", |i| {
            let p = get_extra(i, "ucr").unwrap_or(Obj::proc_(vec![]));
            i.push(p);
            Ok(())
        }),
        op("setcolorrendering", |i| set_extra(i, "crd", 1)),
        op("currentcolorrendering", |i| {
            let p = get_extra(i, "crd").unwrap_or(Obj::Dict(new_dict(4)));
            i.push(p);
            Ok(())
        }),
        op("nulldevice", |i| {
            i.gs.null_device = true;
            i.gs.ctm = Affine::IDENTITY;
            Ok(())
        }),
        // ------------------------------------------------ matrices
        op("matrix", |i| {
            i.push(mat_obj(Affine::IDENTITY));
            Ok(())
        }),
        op("identmatrix", |i| {
            let a = i.pop_arr()?;
            mat_write(&a, Affine::IDENTITY)?;
            i.push(Obj::Array(a, false));
            Ok(())
        }),
        op("initmatrix", |i| {
            i.gs.ctm = Affine::IDENTITY;
            Ok(())
        }),
        op("defaultmatrix", |i| {
            let a = i.pop_arr()?;
            mat_write(&a, Affine::IDENTITY)?;
            i.push(Obj::Array(a, false));
            Ok(())
        }),
        op("currentmatrix", |i| {
            let a = i.pop_arr()?;
            mat_write(&a, i.gs.ctm)?;
            i.push(Obj::Array(a, false));
            Ok(())
        }),
        op("setmatrix", |i| {
            let a = i.pop_arr()?;
            i.gs.ctm = mat_read(&a)?;
            Ok(())
        }),
        op("translate", |i| {
            let m = opt_matrix(i)?;
            let v = i.nums(2)?;
            let t = Affine::translate((v[0], v[1]));
            apply_or_write(i, m, t)
        }),
        op("scale", |i| {
            let m = opt_matrix(i)?;
            let v = i.nums(2)?;
            let t = Affine::scale_non_uniform(v[0], v[1]);
            apply_or_write(i, m, t)
        }),
        op("rotate", |i| {
            let m = opt_matrix(i)?;
            let a = i.pop_num()?;
            let t = Affine::rotate(a.to_radians());
            apply_or_write(i, m, t)
        }),
        op("concat", |i| {
            let a = i.pop_arr()?;
            let m = mat_read(&a)?;
            i.gs.ctm *= m;
            Ok(())
        }),
        op("concatmatrix", |i| {
            let c = i.pop_arr()?;
            let b = i.pop_arr()?;
            let a = i.pop_arr()?;
            let m = mat_read(&b)? * mat_read(&a)?;
            mat_write(&c, m)?;
            i.push(Obj::Array(c, false));
            Ok(())
        }),
        op("invertmatrix", |i| {
            let b = i.pop_arr()?;
            let a = i.pop_arr()?;
            let m = mat_read(&a)?;
            if m.determinant().abs() < 1e-300 {
                return Err(PsErr::UndefinedResult.into());
            }
            mat_write(&b, m.inverse())?;
            i.push(Obj::Array(b, false));
            Ok(())
        }),
        op("transform", |i| xform(i, false, false)),
        op("dtransform", |i| xform(i, true, false)),
        op("itransform", |i| xform(i, false, true)),
        op("idtransform", |i| xform(i, true, true)),
        // ------------------------------------------------ path construction
        op("newpath", |i| {
            i.gs.newpath();
            Ok(())
        }),
        op("currentpoint", |i| {
            let p = i.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
            let u = inv(i)? * p;
            i.push(Obj::Real(u.x));
            i.push(Obj::Real(u.y));
            Ok(())
        }),
        op("moveto", |i| {
            let v = i.nums(2)?;
            let p = i.gs.ctm * Point::new(v[0], v[1]);
            i.moveto(p);
            Ok(())
        }),
        op("rmoveto", |i| {
            let v = i.nums(2)?;
            let c = i.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
            let p = c + dvec(i.gs.ctm, v[0], v[1]);
            i.moveto(p);
            Ok(())
        }),
        op("lineto", |i| {
            let v = i.nums(2)?;
            let p = i.gs.ctm * Point::new(v[0], v[1]);
            i.lineto(p)
        }),
        op("rlineto", |i| {
            let v = i.nums(2)?;
            let c = i.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
            let p = c + dvec(i.gs.ctm, v[0], v[1]);
            i.lineto(p)
        }),
        op("curveto", |i| {
            let v = i.nums(6)?;
            let m = i.gs.ctm;
            i.curveto(m * Point::new(v[0], v[1]), m * Point::new(v[2], v[3]), m * Point::new(v[4], v[5]))
        }),
        op("rcurveto", |i| {
            let v = i.nums(6)?;
            let c = i.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
            let m = i.gs.ctm;
            i.curveto(c + dvec(m, v[0], v[1]), c + dvec(m, v[2], v[3]), c + dvec(m, v[4], v[5]))
        }),
        op("closepath", |i| {
            i.closepath();
            Ok(())
        }),
        op("arc", |i| {
            let v = i.nums(5)?;
            i.arc(v[0], v[1], v[2], v[3], v[4], true)
        }),
        op("arcn", |i| {
            let v = i.nums(5)?;
            i.arc(v[0], v[1], v[2], v[3], v[4], false)
        }),
        op("arct", |i| {
            let v = i.nums(5)?;
            i.arct(v[0], v[1], v[2], v[3], v[4]).map(|_| ())
        }),
        op("arcto", |i| {
            let v = i.nums(5)?;
            let t = i.arct(v[0], v[1], v[2], v[3], v[4])?;
            for x in t {
                i.push(Obj::Real(x));
            }
            Ok(())
        }),
        op("flattenpath", |_| Ok(())),
        op("reversepath", |i| {
            i.gs.path = reverse_subpaths(&i.gs.path);
            Ok(())
        }),
        op("strokepath", |i| {
            if i.gs.path.elements().is_empty() {
                return Ok(());
            }
            let o = i.outline(None);
            let p = i.gs.path.clone();
            i.gs.stroked = Some((p, o));
            Ok(())
        }),
        op("pathbbox", |i| {
            if i.gs.path.elements().is_empty() {
                return Err(PsErr::NoCurrentPoint.into());
            }
            let m = inv(i)?;
            let b = (m * i.gs.path.clone()).bounding_box();
            for v in [b.x0, b.y0, b.x1, b.y1] {
                i.push(Obj::Real(v));
            }
            Ok(())
        }),
        op("pathforall", |i| {
            let close = i.pop_proc()?;
            let curve = i.pop_proc()?;
            let line = i.pop_proc()?;
            let mv = i.pop_proc()?;
            let m = inv(i)?;
            let els: Vec<PathEl> = i.gs.path.elements().to_vec();
            for el in els {
                let (pts, p): (Vec<Point>, &Obj) = match el {
                    PathEl::MoveTo(a) => (vec![a], &mv),
                    PathEl::LineTo(a) => (vec![a], &line),
                    PathEl::QuadTo(a, b) => (vec![a, b, b], &curve),
                    PathEl::CurveTo(a, b, c) => (vec![a, b, c], &curve),
                    PathEl::ClosePath => (vec![], &close),
                };
                for q in pts {
                    let u = m * q;
                    i.push(Obj::Real(u.x));
                    i.push(Obj::Real(u.y));
                }
                match i.exec_value(p.clone()) {
                    Err(Flow::Exit) => break,
                    r => r?,
                }
            }
            Ok(())
        }),
        op("clippath", |i| {
            let p = match &i.gs.clip {
                Some(c) => c.path.clone(),
                None => i.page_rect().to_path(0.1),
            };
            i.gs.newpath();
            i.gs.path = p;
            Ok(())
        }),
        op("setbbox", |i| {
            i.nums(4)?;
            Ok(())
        }),
        op("ucache", |_| Ok(())),
        op("uappend", |i| {
            let a = i.pop_arr()?;
            run_userpath(i, a)
        }),
        op("upath", |i| {
            i.pop()?;
            i.push(Obj::proc_(vec![]));
            Ok(())
        }),
        op("ufill", |i| user_paint(i, 0)),
        op("ueofill", |i| user_paint(i, 1)),
        op("ustroke", |i| {
            // an optional matrix after the user path
            if let Some(Obj::Array(a, false)) = i.os.last() {
                if a.len == 6 && i.os.len() >= 2 && matches!(i.os[i.os.len() - 2], Obj::Array(..)) {
                    let m = mat_read(&a.clone())?;
                    i.pop()?;
                    i.gsave();
                    let up = i.pop_arr()?;
                    i.gs.newpath();
                    run_userpath(i, up)?;
                    i.gs.ctm *= m;
                    let r = i.stroke();
                    i.grestore();
                    return r;
                }
            }
            user_paint(i, 2)
        }),
        // ------------------------------------------------ painting and clipping
        op("fill", |i| i.fill(false)),
        op("eofill", |i| i.fill(true)),
        op("stroke", |i| i.stroke()),
        op("rectfill", |i| {
            let p = rect_operand(i)?;
            i.gsave();
            i.gs.newpath();
            i.gs.path = p;
            let r = i.fill(false);
            i.grestore();
            r
        }),
        op("rectstroke", |i| {
            // a matrix after the rectangles (rectangle lists never hold six numbers)
            let m = match i.os.last() {
                Some(Obj::Array(a, _)) if a.len == 6 => {
                    let a = a.clone();
                    i.pop()?;
                    Some(mat_read(&a)?)
                }
                _ => None,
            };
            let p = rect_operand(i)?;
            i.gsave();
            i.gs.newpath();
            i.gs.path = p;
            if let Some(m) = m {
                i.gs.ctm *= m;
            }
            let r = i.stroke();
            i.grestore();
            r
        }),
        op("rectclip", |i| {
            let p = rect_operand(i)?;
            i.gs.newpath();
            i.gs.path = p;
            i.clip(false);
            i.gs.newpath();
            Ok(())
        }),
        op("clip", |i| {
            i.clip(false);
            Ok(())
        }),
        op("eoclip", |i| {
            i.clip(true);
            Ok(())
        }),
        op("initclip", |i| {
            i.gs.clip = None;
            Ok(())
        }),
        op("erasepage", |_| Ok(())),
        op("infill", |i| inside(i)),
        op("ineofill", |i| inside(i)),
        op("instroke", |i| inside(i)),
        op("inufill", |i| inside(i)),
        op("inueofill", |i| inside(i)),
        op("inustroke", |i| inside(i)),
        // ------------------------------------------------ colors
        op("setgray", |i| {
            let g = i.pop_num()?;
            i.set_color(PsColor::gray(g.clamp(0.0, 1.0)));
            Ok(())
        }),
        op("currentgray", |i| {
            let c = i.gs.color.clone();
            let rgb = i.rgb(&c.space, &c.comps);
            i.push(Obj::Real((0.3 * rgb.r as f64 + 0.59 * rgb.g as f64 + 0.11 * rgb.b as f64) / 255.0));
            Ok(())
        }),
        op("setrgbcolor", |i| {
            let v = i.nums(3)?;
            i.set_color(PsColor { space: CSpace::Rgb, space_obj: None, comps: v.iter().map(|x| x.clamp(0.0, 1.0)).collect(), pattern: None });
            Ok(())
        }),
        op("currentrgbcolor", |i| {
            let c = i.gs.color.clone();
            let rgb = match c.space {
                CSpace::Rgb => {
                    for v in &c.comps {
                        i.push(Obj::Real(*v));
                    }
                    return Ok(());
                }
                _ => i.rgb(&c.space, &c.comps),
            };
            for v in [rgb.r, rgb.g, rgb.b] {
                i.push(Obj::Real(v as f64 / 255.0));
            }
            Ok(())
        }),
        op("sethsbcolor", |i| {
            let v = i.nums(3)?;
            let c = Color::from_hsb(v[0] * 360.0, v[1] * 100.0, v[2] * 100.0);
            i.set_color(PsColor { space: CSpace::Rgb, space_obj: None, comps: vec![c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0], pattern: None });
            Ok(())
        }),
        op("currenthsbcolor", |i| {
            let c = i.gs.color.clone();
            let rgb = i.rgb(&c.space, &c.comps);
            let h = rgb.to_hsb();
            i.push(Obj::Real(h[0] / 360.0));
            i.push(Obj::Real(h[1] / 100.0));
            i.push(Obj::Real(h[2] / 100.0));
            Ok(())
        }),
        op("setcmykcolor", |i| {
            let v = i.nums(4)?;
            i.set_color(PsColor { space: CSpace::Cmyk, space_obj: None, comps: v.iter().map(|x| x.clamp(0.0, 1.0)).collect(), pattern: None });
            Ok(())
        }),
        op("currentcmykcolor", |i| {
            let c = i.gs.color.clone();
            match &c.space {
                CSpace::Cmyk => {
                    for v in &c.comps {
                        i.push(Obj::Real(*v));
                    }
                }
                CSpace::Gray => {
                    for v in [0.0, 0.0, 0.0, 1.0 - c.comps.first().copied().unwrap_or(0.0)] {
                        i.push(Obj::Real(v));
                    }
                }
                _ => {
                    let rgb = i.rgb(&c.space, &c.comps);
                    let k = rgb.to_cmyk();
                    for v in k {
                        i.push(Obj::Real(v / 100.0));
                    }
                }
            }
            Ok(())
        }),
        op("setcolorspace", |i| {
            let o = i.pop()?;
            let space = i.parse_space(&o)?;
            let comps = space.initial();
            i.register_spot(&space);
            i.set_color(PsColor { space, space_obj: Some(o), comps, pattern: None });
            Ok(())
        }),
        op("currentcolorspace", |i| {
            let o = match &i.gs.color.space_obj {
                Some(o @ Obj::Array(..)) => o.clone(),
                Some(o) => Obj::arr(vec![o.clone()]),
                None => {
                    let n = match i.gs.color.space {
                        CSpace::Rgb => "DeviceRGB",
                        CSpace::Cmyk => "DeviceCMYK",
                        _ => "DeviceGray",
                    };
                    let n = i.lit(n);
                    Obj::arr(vec![n])
                }
            };
            i.push(o);
            Ok(())
        }),
        op("setcolor", |i| {
            let space = i.gs.color.space.clone();
            if let CSpace::Pattern(under) = &space {
                let p = i.pop_dict()?;
                let n = under.as_ref().map(|u| u.n()).unwrap_or(0);
                let comps = if n > 0 && i.os.len() >= n { i.nums(n)? } else { vec![] };
                i.gs.color.comps = comps;
                i.gs.color.pattern = Some(p);
                return Ok(());
            }
            let n = space.n();
            let v = i.nums(n)?;
            i.gs.color.comps = v;
            i.gs.color.pattern = None;
            Ok(())
        }),
        op("currentcolor", |i| {
            let c = i.gs.color.clone();
            for v in &c.comps {
                i.push(Obj::Real(*v));
            }
            if let Some(p) = c.pattern {
                i.push(Obj::Dict(p));
            }
            Ok(())
        }),
        op("setpattern", |i| {
            let p = i.pop_dict()?;
            // an uncolored pattern takes the color below it
            let paint = i.dnum(&p, "PaintType").unwrap_or(1.0) as i64;
            let under = if paint == 2 { Some(Box::new(i.gs.color.space.clone())) } else { None };
            let comps = if paint == 2 {
                let n = under.as_ref().map(|u| u.n()).unwrap_or(0);
                if i.os.len() >= n { i.nums(n)? } else { vec![] }
            } else {
                vec![]
            };
            i.gs.color = PsColor { space: CSpace::Pattern(under), space_obj: None, comps, pattern: Some(p) };
            Ok(())
        }),
        op("findcmykcustomcolor", |i| {
            let name = i.pop()?;
            let v = i.nums(4)?;
            let mut items: Vec<Obj> = v.into_iter().map(Obj::Real).collect();
            items.push(name);
            i.push(Obj::arr(items));
            Ok(())
        }),
        op("findrgbcustomcolor", |i| {
            let name = i.pop()?;
            let v = i.nums(3)?;
            let mut items: Vec<Obj> = v.into_iter().map(Obj::Real).collect();
            items.push(name);
            i.push(Obj::arr(items));
            Ok(())
        }),
        op("setcustomcolor", |i| {
            let tint = i.pop_num()?;
            let a = i.pop_arr()?;
            let items = a.items();
            let name = items.last().and_then(|n| i.text_of(n)).unwrap_or_default();
            let nums: Vec<f64> = items.iter().filter_map(|o| o.num()).collect();
            let (alt, base) = if nums.len() >= 4 { (CSpace::Cmyk, nums[..4].to_vec()) } else { (CSpace::Rgb, nums.clone()) };
            let tint_proc = custom_tint(i, &base, matches!(alt, CSpace::Cmyk));
            let space = CSpace::Sep { name, alt: Box::new(alt), tint: tint_proc };
            i.register_spot(&space);
            i.set_color(PsColor { space, space_obj: None, comps: vec![tint.clamp(0.0, 1.0)], pattern: None });
            Ok(())
        }),
        op("currentcustomcolor", |i| {
            i.push(Obj::Null);
            i.push(Obj::Real(1.0));
            Ok(())
        }),
    ]
}

/// The tint transform of a custom color: `t -> (base * t)` for CMYK, `1 - (1-base) * t` for RGB.
fn custom_tint(i: &mut Interp, base: &[f64], cmyk: bool) -> Obj {
    // { dup b0 mul exch dup b1 mul exch ... pop } leaves the components in order
    let mul = Obj::Op(i.op_index("mul").unwrap_or(0));
    let dup = Obj::Op(i.op_index("dup").unwrap_or(0));
    let exch = Obj::Op(i.op_index("exch").unwrap_or(0));
    let pop = Obj::Op(i.op_index("pop").unwrap_or(0));
    let sub = Obj::Op(i.op_index("sub").unwrap_or(0));
    let mut body = vec![];
    for b in base {
        body.push(dup.clone());
        if cmyk {
            body.push(Obj::Real(*b));
            body.push(mul.clone());
        } else {
            // 1 - (1 - b) * t
            body.push(Obj::Real(1.0 - b));
            body.push(mul.clone());
            body.push(Obj::Real(1.0));
            body.push(exch.clone());
            body.push(sub.clone());
        }
        body.push(exch.clone());
    }
    body.push(pop);
    Obj::proc_(body)
}

fn set_extra(i: &mut Interp, key: &'static str, n: usize) -> R {
    i.need(n)?;
    let at = i.os.len() - n;
    let vals: Vec<Obj> = i.os.drain(at..).collect();
    let v = if n == 1 { vals.into_iter().next().unwrap() } else { Obj::arr(vals) };
    i.gs.extra.retain(|(k, _)| *k != key);
    i.gs.extra.push((key, v));
    Ok(())
}

fn get_extra(i: &Interp, key: &str) -> Option<Obj> {
    i.gs.extra.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
}

fn inside(i: &mut Interp) -> R {
    // the point or user path the program asks about, then `false`
    match i.pop()? {
        Obj::Array(..) => {}
        _ => {
            i.pop()?;
        }
    }
    i.push(Obj::Bool(false));
    Ok(())
}

fn run_userpath(i: &mut Interp, a: PsArr) -> R {
    i.run_proc(&a)
}

fn user_paint(i: &mut Interp, how: u8) -> R {
    let a = i.pop_arr()?;
    i.gsave();
    i.gs.newpath();
    let r = run_userpath(i, a).and_then(|_| match how {
        0 => i.fill(false),
        1 => i.fill(true),
        _ => i.stroke(),
    });
    i.grestore();
    r
}

/// The rectangles of `rectfill`, `rectstroke` and `rectclip`: x y w h, or an array or encoded
/// string of numbers. As a device-space path.
fn rect_operand(i: &mut Interp) -> R<BezPath> {
    let nums: Vec<f64> = match i.os.last() {
        Some(Obj::Array(..)) => i.pop_arr()?.items().iter().filter_map(|o| o.num()).collect(),
        Some(Obj::Str(..)) => {
            let s = i.pop_str()?;
            super::ops_base::decode_number_string(&s.bytes())
        }
        _ => i.nums(4)?,
    };
    let m = i.gs.ctm;
    let mut p = BezPath::new();
    for r in nums.chunks(4) {
        if r.len() < 4 {
            break;
        }
        let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
        p.move_to(m * Point::new(x, y));
        p.line_to(m * Point::new(x + w, y));
        p.line_to(m * Point::new(x + w, y + h));
        p.line_to(m * Point::new(x, y + h));
        p.close_path();
    }
    Ok(p)
}

/// The path with each subpath drawn the other way round.
fn reverse_subpaths(p: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut sub: Vec<PathEl> = vec![];
    let flush = |sub: &mut Vec<PathEl>, out: &mut BezPath| {
        if sub.is_empty() {
            return;
        }
        let closed = matches!(sub.last(), Some(PathEl::ClosePath));
        // the points each segment ends at, from the start
        let mut pts = vec![];
        let mut segs = vec![];
        for el in sub.iter() {
            match *el {
                PathEl::MoveTo(a) => pts.push(a),
                PathEl::LineTo(a) => {
                    segs.push((None, None));
                    pts.push(a);
                }
                PathEl::QuadTo(a, b) => {
                    segs.push((Some(a), None));
                    pts.push(b);
                }
                PathEl::CurveTo(a, b, c) => {
                    segs.push((Some(a), Some(b)));
                    pts.push(c);
                }
                PathEl::ClosePath => {}
            }
        }
        out.move_to(*pts.last().unwrap());
        for k in (0..segs.len()).rev() {
            let to = pts[k];
            match segs[k] {
                (None, _) => out.line_to(to),
                (Some(a), None) => out.quad_to(a, to),
                (Some(a), Some(b)) => out.curve_to(b, a, to),
            }
        }
        if closed {
            out.close_path();
        }
        sub.clear();
    };
    for el in p.elements() {
        if matches!(el, PathEl::MoveTo(_)) {
            flush(&mut sub, &mut out);
        }
        sub.push(*el);
    }
    flush(&mut sub, &mut out);
    out
}

pub fn mat_obj(m: Affine) -> Obj {
    Obj::arr(m.as_coeffs().iter().map(|v| Obj::Real(*v)).collect())
}

pub fn mat_read(a: &PsArr) -> R<Affine> {
    if a.len != 6 {
        return Err(PsErr::RangeCheck.into());
    }
    let mut c = [0.0; 6];
    for (k, v) in c.iter_mut().enumerate() {
        *v = a.get(k).and_then(|o| o.num()).ok_or(Flow::Err(PsErr::TypeCheck))?;
    }
    Ok(Affine::new(c))
}

pub fn mat_write(a: &PsArr, m: Affine) -> R {
    if a.len < 6 {
        return Err(PsErr::RangeCheck.into());
    }
    for (k, v) in m.as_coeffs().iter().enumerate() {
        a.put(k, Obj::Real(*v));
    }
    Ok(())
}

fn opt_matrix(i: &mut Interp) -> R<Option<PsArr>> {
    if let Some(Obj::Array(a, _)) = i.os.last() {
        let a = a.clone();
        i.pop()?;
        return Ok(Some(a));
    }
    Ok(None)
}

fn apply_or_write(i: &mut Interp, m: Option<PsArr>, t: Affine) -> R {
    match m {
        Some(a) => {
            mat_write(&a, t)?;
            i.push(Obj::Array(a, false));
        }
        None => i.gs.ctm *= t,
    }
    Ok(())
}

fn xform(i: &mut Interp, delta: bool, inverse: bool) -> R {
    let m = match opt_matrix(i)? {
        Some(a) => mat_read(&a)?,
        None => i.gs.ctm,
    };
    let m = if inverse {
        if m.determinant().abs() < 1e-300 {
            return Err(PsErr::UndefinedResult.into());
        }
        m.inverse()
    } else {
        m
    };
    let v = i.nums(2)?;
    let p = if delta { Point::new(0.0, 0.0) + dvec(m, v[0], v[1]) } else { m * Point::new(v[0], v[1]) };
    i.push(Obj::Real(p.x));
    i.push(Obj::Real(p.y));
    Ok(())
}

pub fn dvec(m: Affine, x: f64, y: f64) -> Vec2 {
    let c = m.as_coeffs();
    Vec2::new(c[0] * x + c[2] * y, c[1] * x + c[3] * y)
}

fn inv(i: &Interp) -> R<Affine> {
    if i.gs.ctm.determinant().abs() < 1e-300 {
        return Err(PsErr::UndefinedResult.into());
    }
    Ok(i.gs.ctm.inverse())
}

/// Points per device unit along a line drawn in user space (for line widths).
pub fn ctm_scale(m: Affine) -> f64 {
    m.determinant().abs().sqrt()
}

impl Interp {
    pub fn gsave(&mut self) {
        self.gstack.push(self.gs.clone());
    }

    pub fn grestore(&mut self) {
        let floor = self.saves.last().copied().unwrap_or(0);
        if self.gstack.len() > floor {
            if let Some(g) = self.gstack.pop() {
                self.gs = g;
            }
        } else if let Some(g) = self.gstack.last() {
            // at a save level: restore without popping
            self.gs = g.clone();
        }
    }

    pub fn page_rect(&self) -> Rect {
        let inv = self.out.to_doc.inverse();
        let (w, h) = self.page_size.unwrap_or((612.0, 792.0));
        (inv * Rect::new(0.0, 0.0, w, h).to_path(0.1)).bounding_box()
    }

    pub fn moveto(&mut self, p: Point) {
        self.gs.stroked = None;
        if let Some(PathEl::MoveTo(_)) = self.gs.path.elements().last() {
            let n = self.gs.path.elements().len();
            let mut els = self.gs.path.elements().to_vec();
            els[n - 1] = PathEl::MoveTo(p);
            self.gs.path = BezPath::from_vec(els);
        } else {
            self.gs.path.move_to(p);
        }
        self.gs.cur = Some(p);
        self.gs.start = Some(p);
    }

    pub fn lineto(&mut self, p: Point) -> R {
        if self.gs.cur.is_none() {
            return Err(PsErr::NoCurrentPoint.into());
        }
        self.gs.stroked = None;
        self.gs.path.line_to(p);
        self.gs.cur = Some(p);
        Ok(())
    }

    pub fn curveto(&mut self, a: Point, b: Point, c: Point) -> R {
        if self.gs.cur.is_none() {
            return Err(PsErr::NoCurrentPoint.into());
        }
        self.gs.stroked = None;
        self.gs.path.curve_to(a, b, c);
        self.gs.cur = Some(c);
        Ok(())
    }

    pub fn closepath(&mut self) {
        if self.gs.cur.is_some() {
            if !matches!(self.gs.path.elements().last(), Some(PathEl::ClosePath) | None) {
                self.gs.path.close_path();
            }
            self.gs.cur = self.gs.start;
        }
    }

    /// An arc in user space, counterclockwise (`arc`) or clockwise (`arcn`).
    pub fn arc(&mut self, cx: f64, cy: f64, r: f64, a1: f64, a2: f64, ccw: bool) -> R {
        let (a1, mut a2) = (a1, a2);
        if ccw {
            while a2 < a1 {
                a2 += 360.0;
            }
        } else {
            while a2 > a1 {
                a2 -= 360.0;
            }
        }
        let m = self.gs.ctm;
        let at = |a: f64| Point::new(cx + r * a.to_radians().cos(), cy + r * a.to_radians().sin());
        let p0 = m * at(a1);
        if self.gs.cur.is_some() {
            self.lineto(p0)?;
        } else {
            self.moveto(p0);
        }
        let sweep = a2 - a1;
        let n = ((sweep.abs() / 90.0).ceil() as usize).max(1);
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * (step.to_radians() / 4.0).tan();
        for s in 0..n {
            let t0 = (a1 + step * s as f64).to_radians();
            let t1 = (a1 + step * (s + 1) as f64).to_radians();
            let p0 = Point::new(cx + r * t0.cos(), cy + r * t0.sin());
            let p3 = Point::new(cx + r * t1.cos(), cy + r * t1.sin());
            let c1 = Point::new(p0.x - k * r * t0.sin(), p0.y + k * r * t0.cos());
            let c2 = Point::new(p3.x + k * r * t1.sin(), p3.y - k * r * t1.cos());
            self.curveto(m * c1, m * c2, m * p3)?;
        }
        Ok(())
    }

    /// `arct`: the tangent points (user space).
    pub fn arct(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, r: f64) -> R<[f64; 4]> {
        let cur = self.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
        let p0 = inv(self)? * cur;
        let p1 = Point::new(x1, y1);
        let p2 = Point::new(x2, y2);
        let v1 = p0 - p1;
        let v2 = p2 - p1;
        let cross = v1.x * v2.y - v1.y * v2.x;
        if r == 0.0 || cross.abs() < 1e-12 || v1.hypot() < 1e-12 || v2.hypot() < 1e-12 {
            let m = self.gs.ctm;
            self.lineto(m * p1)?;
            return Ok([x1, y1, x1, y1]);
        }
        let u1 = v1 / v1.hypot();
        let u2 = v2 / v2.hypot();
        let theta = u1.dot(u2).clamp(-1.0, 1.0).acos();
        let d = r / (theta / 2.0).tan();
        let t1 = p1 + u1 * d;
        let t2 = p1 + u2 * d;
        let bis = (u1 + u2) / (u1 + u2).hypot();
        let c = p1 + bis * (r / (theta / 2.0).sin());
        let a1 = (t1.y - c.y).atan2(t1.x - c.x).to_degrees();
        let a2 = (t2.y - c.y).atan2(t2.x - c.x).to_degrees();
        // turning left (counterclockwise) when the cross product of p0->p1 and p1->p2 is positive
        let turn = (p1 - p0).cross(p2 - p1);
        self.arc(c.x, c.y, r, a1, a2, turn > 0.0)?;
        Ok([t1.x, t1.y, t2.x, t2.y])
    }

    pub fn clip(&mut self, even_odd: bool) {
        let path = self.gs.path.clone();
        let parent = self.gs.clip.clone();
        self.gs.clip = Some(Rc::new(ClipNode { path, even_odd, parent }));
    }

    /// The outline the stroke operators draw with now; `color` none means the current color.
    pub fn outline(&mut self, color: Option<Color>) -> Outline {
        let s = ctm_scale(self.gs.ctm);
        let width = self.gs.lw * s;
        let c = color.unwrap_or_else(|| {
            let pc = self.gs.color.clone();
            self.paint_color(&pc)
        });
        let unit = if width > 0.0 { width } else { s.max(1e-9) };
        Outline {
            width,
            color: Some(c),
            join: match self.gs.join {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            },
            cap: match self.gs.cap {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            },
            dash: self.gs.dash.iter().map(|d| d * s / unit).collect(),
            miter_limit: self.gs.miter,
            ..Outline::default()
        }
    }

    pub fn fill(&mut self, even_odd: bool) -> R {
        let path = std::mem::take(&mut self.gs.path);
        let stroked = self.gs.stroked.take();
        self.gs.newpath();
        if let Some(c) = &mut self.charpath_capture {
            c.extend(path.elements().iter().copied());
            return Ok(());
        }
        if self.gs.null_device || path.elements().is_empty() {
            return Ok(());
        }
        if let Some((p, mut o)) = stroked {
            let pc = self.gs.color.clone();
            o.color = Some(self.paint_color(&pc));
            let st = Style { fill: Fill::None, outline: o, ..Style::default() };
            let clip = self.gs.clip.clone();
            self.out.add_path(&p, st, &clip);
            return Ok(());
        }
        let Some(fill) = self.fill_for(&path) else { return Ok(()) };
        let st = Style { fill, outline: Outline::none(), even_odd, overprint_fill: self.gs.overprint, ..Style::default() };
        let clip = self.gs.clip.clone();
        self.out.add_path(&path, st, &clip);
        Ok(())
    }

    pub fn stroke(&mut self) -> R {
        let path = std::mem::take(&mut self.gs.path);
        self.gs.newpath();
        if let Some(c) = &mut self.charpath_capture {
            c.extend(path.elements().iter().copied());
            return Ok(());
        }
        if self.gs.null_device || path.elements().len() < 2 {
            return Ok(());
        }
        if self.is_none_color() {
            return Ok(());
        }
        let o = self.outline(None);
        let st = Style { fill: Fill::None, outline: o, overprint_outline: self.gs.overprint, ..Style::default() };
        let clip = self.gs.clip.clone();
        self.out.add_path(&path, st, &clip);
        Ok(())
    }

    fn is_none_color(&self) -> bool {
        matches!(&self.gs.color.space, CSpace::Sep { name, .. } if name == "None")
    }

    /// The fill for a path in the current color (`None` when it paints nothing).
    pub fn fill_for(&mut self, path: &BezPath) -> Option<Fill> {
        if self.is_none_color() {
            return None;
        }
        let c = self.gs.color.clone();
        if let (CSpace::Pattern(under), Some(p)) = (&c.space, &c.pattern) {
            return Some(self.pattern_fill(p.clone(), under.as_deref(), &c.comps, path));
        }
        Some(Fill::Solid(self.rgb(&c.space, &c.comps)))
    }

    /// The current color as one color (a pattern gives its main color).
    pub fn paint_color(&mut self, c: &PsColor) -> Color {
        if let (CSpace::Pattern(under), Some(p)) = (&c.space, &c.pattern) {
            let path = self.page_rect().to_path(0.1);
            return self.pattern_fill(p.clone(), under.as_deref(), &c.comps, &path).main_color().unwrap_or(Color::BLACK);
        }
        self.rgb(&c.space, &c.comps)
    }

    pub fn set_color(&mut self, c: PsColor) {
        self.gs.color = c;
    }

    pub fn register_spot(&mut self, space: &CSpace) {
        if let CSpace::Sep { name, .. } = space {
            if name != "All" && name != "None" && !name.is_empty() {
                let c = self.rgb(space, &[1.0]);
                let n = name.clone();
                self.out.add_spot(&n, c);
            }
        }
        if let CSpace::DevN { names, alt, tint } = space {
            for (k, n) in names.clone().iter().enumerate() {
                if matches!(n.as_str(), "Cyan" | "Magenta" | "Yellow" | "Black" | "All" | "None") {
                    continue;
                }
                let mut comps = vec![0.0; names.len()];
                comps[k] = 1.0;
                let sp = CSpace::DevN { names: names.clone(), alt: alt.clone(), tint: tint.clone() };
                let c = self.rgb(&sp, &comps);
                self.out.add_spot(n, c);
            }
        }
    }

    /// A color space from its PostScript form.
    pub fn parse_space(&mut self, o: &Obj) -> R<CSpace> {
        let (name, arr) = match o {
            Obj::Name(n, _) => (self.name_str(*n).to_string(), None),
            Obj::Array(a, _) => {
                let first = a.get(0).ok_or(Flow::Err(PsErr::RangeCheck))?;
                (self.text_of(&first).ok_or(Flow::Err(PsErr::TypeCheck))?, Some(a.clone()))
            }
            _ => return Err(PsErr::TypeCheck.into()),
        };
        let at = |k: usize| arr.as_ref().and_then(|a| a.get(k));
        Ok(match name.as_str() {
            "DeviceGray" | "CIEBasedA" | "CalGray" | "G" => CSpace::Gray,
            "DeviceRGB" | "CIEBasedABC" | "CalRGB" | "RGB" => CSpace::Rgb,
            "DeviceCMYK" | "CIEBasedDEFG" | "CMYK" => CSpace::Cmyk,
            "CIEBasedDEF" => CSpace::Rgb,
            "Lab" => CSpace::Lab,
            "ICCBased" => {
                let n = match at(1) {
                    Some(Obj::Dict(d)) => {
                        if let Some(alt) = self.dget(&d, "Alternate") {
                            return self.parse_space(&alt);
                        }
                        self.dnum(&d, "N").unwrap_or(3.0) as usize
                    }
                    _ => 3,
                };
                match n {
                    1 => CSpace::Gray,
                    4 => CSpace::Cmyk,
                    _ => CSpace::Rgb,
                }
            }
            "Separation" => {
                let n = at(1).and_then(|o| self.text_of(&o)).unwrap_or_default();
                let alt = match at(2) {
                    Some(a) => self.parse_space(&a)?,
                    None => CSpace::Gray,
                };
                CSpace::Sep { name: n, alt: Box::new(alt), tint: at(3).unwrap_or(Obj::Null) }
            }
            "DeviceN" => {
                let names = match at(1) {
                    Some(Obj::Array(a, _)) => a.items().iter().filter_map(|o| self.text_of(o)).collect(),
                    _ => vec![],
                };
                let alt = match at(2) {
                    Some(a) => self.parse_space(&a)?,
                    None => CSpace::Gray,
                };
                CSpace::DevN { names, alt: Box::new(alt), tint: at(3).unwrap_or(Obj::Null) }
            }
            "Indexed" | "I" => {
                let base = match at(1) {
                    Some(b) => self.parse_space(&b)?,
                    None => CSpace::Rgb,
                };
                let hival = at(2).and_then(|o| o.num()).unwrap_or(255.0).max(0.0) as usize;
                CSpace::Indexed { base: Box::new(base), hival, lookup: at(3).unwrap_or(Obj::Null) }
            }
            "Pattern" => {
                let under = match at(1) {
                    Some(b) => Some(Box::new(self.parse_space(&b)?)),
                    None => None,
                };
                CSpace::Pattern(under)
            }
            _ => return Err(PsErr::Undefined.into()),
        })
    }

    /// A color in a color space, as display RGB. CMYK goes through CorelDRAW's default
    /// profile, as CorelDRAW shows it, and keeps its inks.
    pub fn rgb(&mut self, space: &CSpace, c: &[f64]) -> Color {
        let g = |k: usize| c.get(k).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        match space {
            CSpace::Gray => Color::rgb(b(g(0)), b(g(0)), b(g(0))),
            CSpace::Rgb => Color::rgb(b(g(0)), b(g(1)), b(g(2))),
            // the inks are kept, so CMYK output writes them as they were
            CSpace::Cmyk => crate::corel::color::cmyk(g(0), g(1), g(2), g(3)).with_cmyk([g(0), g(1), g(2), g(3)].map(|v| v * 100.0)),
            CSpace::Lab => crate::corel::color::lab(c.first().copied().unwrap_or(0.0), c.get(1).copied().unwrap_or(0.0), c.get(2).copied().unwrap_or(0.0)),
            CSpace::Sep { alt, tint, name } => {
                if name == "All" {
                    let t = g(0);
                    return Color::rgb(b(1.0 - t), b(1.0 - t), b(1.0 - t));
                }
                let out = self.run_tint(tint, &c[..c.len().min(1)], alt.n());
                self.rgb(alt, &out)
            }
            CSpace::DevN { alt, tint, .. } => {
                let out = self.run_tint(tint, c, alt.n());
                self.rgb(alt, &out)
            }
            CSpace::Indexed { base, hival, lookup } => {
                let idx = (c.first().copied().unwrap_or(0.0).round().max(0.0) as usize).min(*hival);
                let n = base.n();
                let comps = self.index_lookup(lookup, idx, n, base);
                self.rgb(base, &comps)
            }
            CSpace::Pattern(_) => Color::rgb(128, 128, 128),
        }
    }

    /// The components of entry `idx` of an indexed color space.
    pub fn index_lookup(&mut self, lookup: &Obj, idx: usize, n: usize, base: &CSpace) -> Vec<f64> {
        match lookup {
            Obj::Str(s, _) => {
                let lab = matches!(base, CSpace::Lab);
                (0..n)
                    .map(|k| {
                        let v = s.get(idx * n + k).unwrap_or(0) as f64 / 255.0;
                        if lab {
                            if k == 0 {
                                v * 100.0
                            } else {
                                v * 255.0 - 128.0
                            }
                        } else {
                            v
                        }
                    })
                    .collect()
            }
            p @ Obj::Array(_, _) => self.run_tint(p, &[idx as f64], n),
            _ => vec![0.0; n],
        }
    }

    /// Run a tint transform (or other color procedure): `n` components out.
    pub fn run_tint(&mut self, proc_: &Obj, input: &[f64], n: usize) -> Vec<f64> {
        if !matches!(proc_, Obj::Array(..) | Obj::Op(_) | Obj::Name(..)) {
            return vec![1.0 - input.first().copied().unwrap_or(0.0); n];
        }
        let base = self.os.len();
        for v in input {
            self.push(Obj::Real(*v));
        }
        let ok = self.exec_value(proc_.clone().with_exec(true)).is_ok();
        let mut out = vec![0.0; n];
        if ok && self.os.len() >= base + n {
            let at = self.os.len() - n;
            for (k, o) in self.os[at..].iter().enumerate() {
                out[k] = o.num().unwrap_or(0.0);
            }
        }
        self.os.truncate(base);
        out
    }
}

pub fn rect_path(r: Rect) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((r.x0, r.y0));
    p.line_to((r.x1, r.y0));
    p.line_to((r.x1, r.y1));
    p.line_to((r.x0, r.y1));
    p.close_path();
    p
}
