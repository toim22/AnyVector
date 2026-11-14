//! Patterns and smooth shading: axial and radial shadings become fountain fills; tiling
//! patterns and the other shadings are shown in one color.

use super::gfx::{mat_read, CSpace};
use super::interp::{op, Interp, OpFn};
use super::obj::*;
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use vector_core::color::Color;
use vector_core::style::{Fill, Gradient, GradientKind, GradientStop, Outline, SpreadMode, Style};

pub fn ops() -> Vec<(&'static str, OpFn)> {
    vec![
        op("makepattern", |i| {
            let m = i.pop_arr()?;
            let m = mat_read(&m)?;
            let p = i.pop_dict()?;
            let np = new_dict(p.borrow().cap + 2);
            for (k, v) in p.borrow().map.iter() {
                np.borrow_mut().map.insert(k.clone(), v.clone());
            }
            let ctm = i.gs.ctm;
            i.dput(&np, "AVMatrix", super::gfx::mat_obj(ctm * m));
            i.push(Obj::Dict(np));
            Ok(())
        }),
        op("shfill", |i| {
            let sh = i.pop_dict()?;
            if i.gs.null_device {
                return Ok(());
            }
            let ctm = i.gs.ctm;
            // the area painted: the shading's box, else the clipping path
            let (path, clip) = match i.dget(&sh, "BBox") {
                Some(Obj::Array(b, _)) if b.len == 4 => {
                    let v: Vec<f64> = b.items().iter().filter_map(|o| o.num()).collect();
                    (ctm * super::gfx::rect_path(Rect::new(v[0], v[1], v[2], v[3])), i.gs.clip.clone())
                }
                _ => match i.gs.clip.clone() {
                    Some(c) => (c.path.clone(), c.parent.clone()),
                    None => (i.page_rect().to_path(0.1), None),
                },
            };
            let fill = i.shading_fill(&sh, ctm, &path);
            let st = Style { fill, outline: Outline::none(), even_odd: false, ..Style::default() };
            i.out.add_path(&path, st, &clip);
            Ok(())
        }),
    ]
}

impl Interp {
    /// The fill a pattern paints a path (device space) with.
    pub fn pattern_fill(&mut self, p: DictRef, under: Option<&CSpace>, comps: &[f64], path: &BezPath) -> Fill {
        let m = match self.dget(&p, "AVMatrix") {
            Some(Obj::Array(a, _)) => mat_read(&a).unwrap_or(Affine::IDENTITY),
            _ => Affine::IDENTITY,
        };
        let kind = self.dnum(&p, "PatternType").unwrap_or(1.0) as i64;
        if kind == 2 {
            if let Some(Obj::Dict(sh)) = self.dget(&p, "Shading") {
                return self.shading_fill(&sh, m, path);
            }
            return Fill::Solid(Color::rgb(128, 128, 128));
        }
        self.counts.patterns += 1;
        let paint_type = self.dnum(&p, "PaintType").unwrap_or(1.0) as i64;
        if paint_type == 2 {
            if let Some(u) = under {
                return Fill::Solid(self.rgb(u, comps));
            }
        }
        if self.out.capture.is_some() {
            return Fill::Solid(Color::rgb(128, 128, 128));
        }
        // draw the cell once, keeping only its colors
        let Some(proc_) = self.dget(&p, "PaintProc") else { return Fill::Solid(Color::rgb(128, 128, 128)) };
        self.out.capture = Some(vec![]);
        self.gsave();
        self.gs.ctm = m;
        self.gs.newpath();
        self.push(Obj::Dict(p.clone()));
        let base = self.os.len() - 1;
        let _ = self.exec_value(proc_.with_exec(true));
        self.os.truncate(base);
        self.grestore();
        let colors = self.out.capture.take().unwrap_or_default();
        // the color painted most, white paper aside
        let mut best: Option<(Color, usize)> = None;
        for c in &colors {
            if *c == Color::WHITE && colors.len() > 1 {
                continue;
            }
            let n = colors.iter().filter(|x| *x == c).count();
            if best.is_none_or(|(_, k)| n > k) {
                best = Some((*c, n));
            }
        }
        Fill::Solid(best.map(|b| b.0).unwrap_or(Color::rgb(128, 128, 128)))
    }

    /// A shading as a fountain fill of a path (device space); `m` maps shading space to device
    /// space.
    pub fn shading_fill(&mut self, sh: &DictRef, m: Affine, path: &BezPath) -> Fill {
        let kind = self.dnum(sh, "ShadingType").unwrap_or(0.0) as i64;
        let space = match self.dget(sh, "ColorSpace") {
            Some(o) => self.parse_space(&o).unwrap_or(CSpace::Rgb),
            None => CSpace::Rgb,
        };
        let nums = |i: &mut Interp, k: &str| -> Vec<f64> {
            match i.dget(sh, k) {
                Some(Obj::Array(a, _)) => a.items().iter().filter_map(|o| o.num()).collect(),
                _ => vec![],
            }
        };
        let coords = nums(self, "Coords");
        let domain = {
            let d = nums(self, "Domain");
            if d.len() >= 2 {
                (d[0], d[1])
            } else {
                (0.0, 1.0)
            }
        };
        let func = self.dget(sh, "Function");
        if !(kind == 2 && coords.len() >= 4 || kind == 3 && coords.len() >= 6) || func.is_none() {
            self.counts.shadings += 1;
            // one color: the middle of the function, the background, or gray
            let c = match &func {
                Some(f) => {
                    let v = self.eval_fn(f, (domain.0 + domain.1) / 2.0, space.n());
                    self.rgb(&space, &v)
                }
                None => match nums(self, "Background") {
                    b if !b.is_empty() => self.rgb(&space, &b),
                    _ => Color::rgb(128, 128, 128),
                },
            };
            return Fill::Solid(c);
        }
        let func = func.unwrap();
        // stops sampled along the domain, then thinned where they are in line
        let n = 64;
        let mut stops: Vec<GradientStop> = (0..=n)
            .map(|k| {
                let s = k as f64 / n as f64;
                let t = domain.0 + (domain.1 - domain.0) * s;
                let v = self.eval_fn(&func, t, space.n());
                GradientStop { pos: s, color: self.rgb(&space, &v) }
            })
            .collect();
        thin(&mut stops);
        let doc = self.out.to_doc;
        let bb = (doc * path.clone()).bounding_box();
        let unit = |p: Point| Point::new((p.x - bb.x0) / bb.width().max(1e-9), (p.y - bb.y0) / bb.height().max(1e-9));
        let at = |x: f64, y: f64| unit(doc * (m * Point::new(x, y)));
        if kind == 2 {
            let (a, b) = (at(coords[0], coords[1]), at(coords[2], coords[3]));
            if (a - b).hypot() < 1e-9 {
                return Fill::Solid(stops[0].color);
            }
            return Fill::Gradient(Gradient { kind: GradientKind::Linear, stops, start: a, end: b, spread: SpreadMode::Pad, steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true });
        }
        // radial: from the end circle's center; an inner radius moves the stops out
        let (r0, r1) = (coords[2].abs(), coords[5].abs());
        let (center, rad, inner) = if r1 >= r0 { ((coords[3], coords[4]), r1, r0) } else { ((coords[0], coords[1]), r0, r1) };
        if r1 < r0 {
            // the larger circle first: reverse the stops
            stops.reverse();
            for s in stops.iter_mut() {
                s.pos = 1.0 - s.pos;
            }
        }
        if rad > 1e-12 && inner > 0.0 {
            let k = inner / rad;
            for s in stops.iter_mut() {
                s.pos = k + s.pos * (1.0 - k);
            }
        }
        let c = at(center.0, center.1);
        let e = at(center.0 + rad, center.1);
        Fill::Gradient(Gradient { kind: GradientKind::Elliptical, stops, start: c, end: e, spread: SpreadMode::Pad, steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true })
    }

    /// Evaluate a function (dictionary, or an array of one-output functions).
    pub fn eval_fn(&mut self, f: &Obj, t: f64, n: usize) -> Vec<f64> {
        match f {
            Obj::Array(a, false) => a.items().iter().map(|g| self.eval_fn(g, t, 1).first().copied().unwrap_or(0.0)).collect(),
            Obj::Dict(d) => {
                let d = d.clone();
                let nums = |i: &mut Interp, k: &str| -> Vec<f64> {
                    match i.dget(&d, k) {
                        Some(Obj::Array(a, _)) => a.items().iter().filter_map(|o| o.num()).collect(),
                        _ => vec![],
                    }
                };
                let dom = nums(self, "Domain");
                let t = if dom.len() >= 2 { t.clamp(dom[0].min(dom[1]), dom[0].max(dom[1])) } else { t };
                let kind = self.dnum(&d, "FunctionType").unwrap_or(2.0) as i64;
                match kind {
                    2 => {
                        let c0 = nums(self, "C0");
                        let c1 = nums(self, "C1");
                        let c0 = if c0.is_empty() { vec![0.0] } else { c0 };
                        let c1 = if c1.is_empty() { vec![1.0] } else { c1 };
                        let e = self.dnum(&d, "N").unwrap_or(1.0);
                        let s = if e == 1.0 { t } else { t.max(0.0).powf(e) };
                        c0.iter().zip(&c1).map(|(a, b)| a + s * (b - a)).collect()
                    }
                    3 => {
                        let funcs = match self.dget(&d, "Functions") {
                            Some(Obj::Array(a, _)) => a.items(),
                            _ => vec![],
                        };
                        if funcs.is_empty() {
                            return vec![0.0; n];
                        }
                        let bounds = nums(self, "Bounds");
                        let enc = nums(self, "Encode");
                        let (d0, d1) = if dom.len() >= 2 { (dom[0], dom[1]) } else { (0.0, 1.0) };
                        let mut k = 0;
                        while k < bounds.len() && t >= bounds[k] {
                            k += 1;
                        }
                        let k = k.min(funcs.len() - 1);
                        let lo = if k == 0 { d0 } else { bounds[k - 1] };
                        let hi = if k < bounds.len() { bounds[k] } else { d1 };
                        let (e0, e1) = (enc.get(2 * k).copied().unwrap_or(0.0), enc.get(2 * k + 1).copied().unwrap_or(1.0));
                        let s = if (hi - lo).abs() < 1e-12 { e0 } else { e0 + (t - lo) / (hi - lo) * (e1 - e0) };
                        let g = funcs[k].clone();
                        self.eval_fn(&g, s, n)
                    }
                    0 => {
                        let size = nums(self, "Size").first().copied().unwrap_or(2.0).max(1.0) as usize;
                        let bps = self.dnum(&d, "BitsPerSample").unwrap_or(8.0) as usize;
                        let range = nums(self, "Range");
                        let outs = (range.len() / 2).max(1);
                        let enc = nums(self, "Encode");
                        let dec = {
                            let v = nums(self, "Decode");
                            if v.is_empty() {
                                range.clone()
                            } else {
                                v
                            }
                        };
                        let data = match self.dget(&d, "DataSource") {
                            Some(Obj::Str(s, _)) => s.bytes(),
                            Some(Obj::File(f, _)) => {
                                super::filters::ensure(&f);
                                let b = f.borrow();
                                b.data[b.pos.min(b.data.len())..].to_vec()
                            }
                            _ => vec![],
                        };
                        let (d0, d1) = if dom.len() >= 2 { (dom[0], dom[1]) } else { (0.0, 1.0) };
                        let (e0, e1) = (enc.first().copied().unwrap_or(0.0), enc.get(1).copied().unwrap_or(size as f64 - 1.0));
                        let e = if (d1 - d0).abs() < 1e-12 { e0 } else { e0 + (t - d0) / (d1 - d0) * (e1 - e0) };
                        let e = e.clamp(0.0, size as f64 - 1.0);
                        let (i0, frac) = (e.floor() as usize, e - e.floor());
                        let i1 = (i0 + 1).min(size - 1);
                        let max = ((1u64 << bps.min(32)) - 1) as f64;
                        let sample = |idx: usize, o: usize| -> f64 {
                            let bit = (idx * outs + o) * bps;
                            let mut v = 0u64;
                            for b in 0..bps {
                                let at = bit + b;
                                let byte = data.get(at / 8).copied().unwrap_or(0);
                                v = v << 1 | ((byte >> (7 - at % 8)) & 1) as u64;
                            }
                            v as f64
                        };
                        (0..outs)
                            .map(|o| {
                                let v = sample(i0, o) * (1.0 - frac) + sample(i1, o) * frac;
                                let (lo, hi) = (dec.get(2 * o).copied().unwrap_or(0.0), dec.get(2 * o + 1).copied().unwrap_or(1.0));
                                lo + v / max * (hi - lo)
                            })
                            .collect()
                    }
                    _ => {
                        // a procedure body (FunctionType 4 as some writers store it)
                        if let Some(p) = self.dget(&d, "AVProc") {
                            return self.run_tint(&p, &[t], n);
                        }
                        vec![0.0; n]
                    }
                }
            }
            p @ Obj::Array(_, true) => self.run_tint(p, &[t], n),
            _ => vec![0.0; n],
        }
    }
}

/// Drop stops that lie on the line between their neighbors.
fn thin(stops: &mut Vec<GradientStop>) {
    let mut k = 1;
    while k + 1 < stops.len() {
        let (a, b, c) = (stops[k - 1], stops[k], stops[k + 1]);
        let t = (b.pos - a.pos) / (c.pos - a.pos).max(1e-12);
        let mid = Color::lerp(a.color, c.color, t);
        let off = (mid.r as i32 - b.color.r as i32).abs().max((mid.g as i32 - b.color.g as i32).abs()).max((mid.b as i32 - b.color.b as i32).abs());
        if off <= 1 {
            stops.remove(k);
        } else {
            k += 1;
        }
    }
}
