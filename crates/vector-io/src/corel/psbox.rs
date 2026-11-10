//! CorelDRAW's PostScript fills that paint the object's own box rather than repeat a tile.
//!
//! CorelDRAW draws a PostScript fill by running a procedure from its library with the fill's
//! numbers and the object's bounding box (`Bbllx Bblly Bburx Bbury`, in mils: thousandths of an
//! inch). Most of the procedures place their marks from that box: rows of a pattern that start at
//! its lower left corner, spokes from a point set in percent of it, rings around its middle, leaves
//! scattered over it. Each fill is drawn again here, in Rust, with the same geometry, on a
//! [`Canvas`] that follows PostScript's painting model (a current transform, a current path in
//! device space, gray or RGB color, line width, `gsave`/`grestore`, and glyphs stamped from the
//! current point the way a Type 3 font's `show` does).
//!
//! The scattered fills use PostScript's `rand`. CorelDRAW's own sequence isn't known; the generator
//! here is the Park-Miller one most PostScript interpreters use, seeded with the fill's "Random
//! seed" number (or, for fills without one, from all of its numbers), so a drawing always looks the
//! same: the scatter has CorelDRAW's density, sizes and colors range, not its exact positions. The
//! color fills pick their colors with the same generator.

use kurbo::{Affine, PathEl, Point};
use tiny_skia as sk;

/// Most marks a fill may make before it is given up on (a tone is used instead): keeps a fill on a
/// huge object, or one whose numbers make it loop forever, from hanging the import.
const BUDGET: usize = 3_000_000;

/// How far CorelDRAW's box for a fill (`Bbllx`...) reaches past the object's box on each side,
/// in mils: about a point, measured from its renders (the corner the Spokes fill radiates from
/// is that far outside the object, at any resolution).
const MARGIN: f64 = 13.9;

/// How much wider, in inches, CorelDRAW's lines and shapes come out than they are: PostScript
/// paints every pixel a shape touches, which adds about half a pixel (at 150 dpi, as CorelDRAW
/// shows and exports them).
const SPREAD: f64 = 1.0 / 300.0;

/// The thinnest line drawn, in inches: a pixel of the picture, not a faint hairline.
const THINNEST: f64 = 1.0 / 300.0;

/// A PostScript-like drawing surface over an object's box: user space starts as mils with the
/// origin at the lower left corner of CorelDRAW's box for the fill, y up.
pub(super) struct Canvas {
    pm: sk::Pixmap,
    gs: Gs,
    saved: Vec<Gs>,
    /// The width and height in mils of CorelDRAW's box for the fill (`Bburx - Bbllx`,
    /// `Bbury - Bblly`).
    w: f64,
    h: f64,
    /// [`SPREAD`] and [`THINNEST`] in pixels.
    spread: f32,
    thinnest: f32,
    seed: u32,
    /// Strokes of one color and width, waiting to be drawn together.
    pending: Option<(Vec<PathEl>, [u8; 3], f32)>,
    budget: usize,
}

#[derive(Clone)]
struct Gs {
    ctm: Affine,
    /// The current path, in device pixels.
    path: Vec<PathEl>,
    cur: Option<Point>,
    start: Option<Point>,
    color: [u8; 3],
    lw: f64,
}

/// A device gray for a PostScript gray (0 black, 1 white): CorelDRAW shows grays lighter than the
/// ink alone, the way ink spreads on paper (40% black shows as 172 of 255).
fn shade(g: f64) -> u8 {
    (255.0 * g.clamp(0.0, 1.0).powf(0.77)).round() as u8
}

impl Canvas {
    /// A canvas for an object's box `w` × `h` mils, `ppm` pixels per mil.
    pub(super) fn new(w: f64, h: f64, ppm: f64, seed: u32) -> Option<Canvas> {
        let pw = (w * ppm).ceil().max(1.0) as u32;
        let ph = (h * ppm).ceil().max(1.0) as u32;
        let pm = sk::Pixmap::new(pw, ph)?;
        let m = MARGIN;
        let ctm = Affine::new([ppm, 0.0, 0.0, -ppm, -m * ppm, (h + m) * ppm]);
        let gs = Gs { ctm, path: vec![], cur: None, start: None, color: [0, 0, 0], lw: 1.0 };
        let (spread, thinnest) = ((SPREAD * 1000.0 * ppm) as f32, (THINNEST * 1000.0 * ppm) as f32);
        let mut c = Canvas { pm, gs, saved: vec![], w: w + 2.0 * m, h: h + 2.0 * m, spread, thinnest, seed: 1, pending: None, budget: BUDGET };
        c.srand(seed as i64);
        Some(c)
    }

    /// The picture: straight (not premultiplied) RGBA, its width and height in pixels; `None` when
    /// the fill ran out of its budget.
    pub(super) fn finish(mut self) -> Option<(Vec<u8>, u32, u32)> {
        self.flush();
        if self.over() {
            return None;
        }
        let (w, h) = (self.pm.width(), self.pm.height());
        let rgba = self.pm.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect();
        Some((rgba, w, h))
    }

    fn over(&self) -> bool {
        self.budget == 0
    }

    fn spend(&mut self, n: usize) {
        self.budget = self.budget.saturating_sub(n);
    }

    // ---- random numbers: `srand` and `rand`, Park-Miller as in most PostScript interpreters

    fn srand(&mut self, seed: i64) {
        const M: i64 = 0x7fff_ffff;
        let s = if seed < 1 { -(seed % (M - 1)) + 1 } else { seed.min(M - 1) };
        self.seed = s as u32;
    }

    fn rand(&mut self) -> i64 {
        self.seed = ((self.seed as u64 * 16807) % 0x7fff_ffff) as u32;
        self.seed as i64
    }

    /// `rand n mod`
    fn rmod(&mut self, n: f64) -> f64 {
        let n = (n as i64).max(1);
        (self.rand() % n) as f64
    }

    /// A random color, as the color fills' `SetRandomRGB` picks one.
    fn random_rgb(&mut self) {
        let mut v = [0.0; 3];
        for c in &mut v {
            *c = 1.0 - (self.rmod(100.0) + 1.0) / 100.0;
        }
        self.setrgb(v[0], v[1], v[2]);
    }

    // ---- graphics state

    fn gsave(&mut self) {
        self.saved.push(self.gs.clone());
    }

    fn grestore(&mut self) {
        if let Some(g) = self.saved.pop() {
            self.gs = g;
        }
    }

    fn ctm(&self) -> Affine {
        self.gs.ctm
    }

    fn set_ctm(&mut self, m: Affine) {
        self.gs.ctm = m;
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.gs.ctm *= Affine::translate((x, y));
    }

    fn rotate(&mut self, deg: f64) {
        self.gs.ctm *= Affine::rotate(deg.to_radians());
    }

    fn scale(&mut self, sx: f64, sy: f64) {
        self.gs.ctm *= Affine::scale_non_uniform(sx, sy);
    }

    fn concat(&mut self, m: [f64; 6]) {
        self.gs.ctm *= Affine::new(m);
    }

    /// `setgray`: 0 black, 1 white.
    fn setgray(&mut self, g: f64) {
        let v = shade(g);
        self.gs.color = [v, v, v];
    }

    /// The gray of a fill's "gray" number, in percent of black.
    fn ink(&mut self, percent: f64) {
        self.setgray(1.0 - percent / 100.0);
    }

    fn setrgb(&mut self, r: f64, g: f64, b: f64) {
        let v = |c: f64| (255.0 * c.clamp(0.0, 1.0)).round() as u8;
        self.gs.color = [v(r), v(g), v(b)];
    }

    fn setlinewidth(&mut self, w: f64) {
        self.gs.lw = w;
    }

    // ---- path

    fn dev(&self, x: f64, y: f64) -> Point {
        self.gs.ctm * Point::new(x, y)
    }

    fn moveto_dev(&mut self, p: Point) {
        // a move right after a move replaces it, as in PostScript
        if let Some(PathEl::MoveTo(_)) = self.gs.path.last() {
            self.gs.path.pop();
        }
        self.gs.path.push(PathEl::MoveTo(p));
        self.gs.cur = Some(p);
        self.gs.start = Some(p);
    }

    fn moveto(&mut self, x: f64, y: f64) {
        let p = self.dev(x, y);
        self.moveto_dev(p);
    }

    fn lineto(&mut self, x: f64, y: f64) {
        let p = self.dev(x, y);
        if self.gs.cur.is_none() {
            return self.moveto_dev(p);
        }
        self.gs.path.push(PathEl::LineTo(p));
        self.gs.cur = Some(p);
        self.spend(1);
    }

    fn rlineto(&mut self, dx: f64, dy: f64) {
        let (x, y) = self.currentpoint();
        self.lineto(x + dx, y + dy);
    }

    fn curveto(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let (a, b, c) = (self.dev(x1, y1), self.dev(x2, y2), self.dev(x3, y3));
        if self.gs.cur.is_none() {
            self.moveto_dev(a);
        }
        self.gs.path.push(PathEl::CurveTo(a, b, c));
        self.gs.cur = Some(c);
        self.spend(1);
    }

    fn closepath(&mut self) {
        if let Some(s) = self.gs.start {
            if self.gs.cur.is_some() {
                self.gs.path.push(PathEl::ClosePath);
                self.gs.cur = Some(s);
            }
        }
    }

    /// `arc` (counterclockwise) and `arcn` (`clockwise`), angles in degrees: a line from the
    /// current point to the arc's start, if there is a current point, then the arc.
    fn arc_dir(&mut self, cx: f64, cy: f64, r: f64, a1: f64, a2: f64, clockwise: bool) {
        let mut sweep = a2 - a1;
        if clockwise {
            while sweep > 0.0 {
                sweep -= 360.0;
            }
        } else {
            while sweep < 0.0 {
                sweep += 360.0;
            }
        }
        let at = |a: f64| Point::new(cx + r * a.to_radians().cos(), cy + r * a.to_radians().sin());
        let s = at(a1);
        if self.gs.cur.is_some() {
            self.lineto(s.x, s.y);
        } else {
            self.moveto(s.x, s.y);
        }
        let n = (sweep.abs() / 90.0).ceil().max(1.0) as usize;
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * (step.to_radians() / 4.0).tan();
        for i in 0..n {
            let (t0, t1) = ((a1 + step * i as f64).to_radians(), (a1 + step * (i + 1) as f64).to_radians());
            let (p0, p3) = (at(a1 + step * i as f64), at(a1 + step * (i + 1) as f64));
            let p1 = Point::new(p0.x - k * r * t0.sin(), p0.y + k * r * t0.cos());
            let p2 = Point::new(p3.x + k * r * t1.sin(), p3.y - k * r * t1.cos());
            self.curveto(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
        }
    }

    fn arc(&mut self, cx: f64, cy: f64, r: f64, a1: f64, a2: f64) {
        self.arc_dir(cx, cy, r, a1, a2, false);
    }

    fn arcn(&mut self, cx: f64, cy: f64, r: f64, a1: f64, a2: f64) {
        self.arc_dir(cx, cy, r, a1, a2, true);
    }

    fn currentpoint(&self) -> (f64, f64) {
        match self.gs.cur {
            Some(p) => {
                let u = self.gs.ctm.inverse() * p;
                (u.x, u.y)
            }
            None => (0.0, 0.0),
        }
    }

    fn newpath(&mut self) {
        self.gs.path.clear();
        self.gs.cur = None;
        self.gs.start = None;
    }

    // ---- painting

    fn sk_path(els: &[PathEl]) -> Option<sk::Path> {
        let mut pb = sk::PathBuilder::new();
        for el in els {
            match *el {
                PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
                PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
                PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
                PathEl::CurveTo(a, b, p) => pb.cubic_to(a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32),
                PathEl::ClosePath => pb.close(),
            }
        }
        pb.finish()
    }

    fn paint(c: [u8; 3]) -> sk::Paint<'static> {
        let mut p = sk::Paint::default();
        p.set_color_rgba8(c[0], c[1], c[2], 255);
        p.anti_alias = true;
        p
    }

    fn flush(&mut self) {
        if let Some((els, c, w)) = self.pending.take() {
            if let Some(path) = Self::sk_path(&els) {
                let st = sk::Stroke { width: w, ..Default::default() };
                self.pm.stroke_path(&path, &Self::paint(c), &st, sk::Transform::identity(), None);
            }
        }
    }

    /// `fill`: the current path, nonzero winding.
    fn fill(&mut self) {
        self.flush();
        if !self.over() {
            if let Some(path) = Self::sk_path(&self.gs.path) {
                let paint = Self::paint(self.gs.color);
                self.pm.fill_path(&path, &paint, sk::FillRule::Winding, sk::Transform::identity(), None);
                // as CorelDRAW's come out (see SPREAD)
                let st = sk::Stroke { width: self.spread, ..Default::default() };
                self.pm.stroke_path(&path, &paint, &st, sk::Transform::identity(), None);
            }
        }
        self.newpath();
    }

    /// `fill` of the object's own path (a background): the whole box, which the object's outline
    /// clips.
    fn fill_box(&mut self) {
        self.flush();
        let [r, g, b] = self.gs.color;
        self.pm.fill(sk::Color::from_rgba8(r, g, b, 255));
        self.newpath();
    }

    fn stroke(&mut self) {
        let [a, b, c, d, _, _] = self.gs.ctm.as_coeffs();
        // as wide as CorelDRAW's come out (see SPREAD)
        let w = ((self.gs.lw * (a * d - b * c).abs().sqrt()) as f32 + self.spread).max(self.thinnest);
        let color = self.gs.color;
        let path = std::mem::take(&mut self.gs.path);
        self.newpath();
        if self.over() {
            return;
        }
        match &mut self.pending {
            Some((els, c, pw)) if *c == color && *pw == w => els.extend(path),
            _ => {
                self.flush();
                self.pending = Some((path, color, w));
            }
        }
        if self.pending.as_ref().is_some_and(|p| p.0.len() > 20_000) {
            self.flush();
        }
    }

    /// `show` of one glyph of a Type 3 font at the current point: `glyph` draws it in glyph space,
    /// `k` glyph units to the user unit (font matrix times point size), then the current point
    /// moves on by the glyph's advance.
    fn show(&mut self, k: f64, adv: (f64, f64), glyph: impl FnOnce(&mut Canvas)) {
        if self.gs.cur.is_none() || self.over() {
            return;
        }
        let (x, y) = self.currentpoint();
        self.gsave();
        self.gs.ctm *= Affine::translate((x, y)) * Affine::scale(k);
        let end = self.gs.ctm * Point::new(adv.0, adv.1);
        self.gs.path.clear();
        self.gs.cur = None;
        self.gs.start = None;
        glyph(self);
        self.grestore();
        self.moveto_dev(end);
        self.spend(1);
    }

    /// Rows of glyphs, as most of the library's pattern fills lay them out: a row starts at `x0`
    /// every `step` from `y0` to `y1`, and glyphs follow each other from there until one ends past
    /// `x1`. With `lift`, a glyph whose advance takes the row higher than its limit above the row's
    /// start drops back by its amount, so that a row zigzags instead of climbing.
    #[allow(clippy::too_many_arguments)]
    fn rows(&mut self, x0: f64, x1: f64, y0: f64, step: f64, y1: f64, lift: Option<(f64, f64)>, mut show: impl FnMut(&mut Canvas)) {
        for y in steps(y0, step, y1) {
            self.moveto(x0, y);
            loop {
                if self.over() {
                    return;
                }
                show(self);
                let (cx, mut cy) = self.currentpoint();
                if let Some((limit, drop)) = lift {
                    if cy - y > limit {
                        cy -= drop;
                    }
                }
                if cx > x1 || self.gs.cur.is_none() {
                    break;
                }
                self.moveto(cx, cy);
            }
        }
    }
}

/// PostScript's `for` over reals: `from`, `from + step`, ... while not past `to`, added up in
/// single precision as PostScript does (so `-.5 .05 .5` stops short of .5, as in CorelDRAW).
fn steps(from: f64, step: f64, to: f64) -> impl Iterator<Item = f64> {
    let ok = step != 0.0 && step.is_finite() && from.is_finite() && to.is_finite();
    let (mut v, step, to) = (from as f32, step as f32, to as f32);
    let mut n = if ok { 0 } else { 1_000_000 - 1 };
    std::iter::from_fn(move || {
        let inside = if step > 0.0 { v <= to } else { v >= to };
        if (!inside && ok) || n >= 1_000_000 {
            return None;
        }
        let out = v as f64;
        v += step;
        n += 1;
        Some(out)
    })
}

/// The fill's numbers, each clamped into its range (CorelDRAW's `InRange`), with the library's
/// default for a number the file doesn't have.
struct Args<'a>(&'a [i32]);

impl Args<'_> {
    fn get(&self, i: usize, default: i32, lo: f64, hi: f64) -> f64 {
        (self.0.get(i).copied().unwrap_or(default) as f64).clamp(lo, hi.max(lo))
    }
    fn raw(&self, i: usize, default: i32) -> i64 {
        self.0.get(i).copied().unwrap_or(default) as i64
    }
}

/// CorelDRAW's `wDstChck`: a maximum equal to its minimum is one more.
fn distinct(max: f64, min: f64) -> f64 {
    if max == min { max + 1.0 } else { max }
}

/// The library's fills by their place in it (1 is Archimedes).
pub(super) mod fill {
    pub const ARCHIMEDES: u16 = 1;
    pub const BARS: u16 = 2;
    pub const BASKETWEAVE: u16 = 3;
    pub const BIRDS: u16 = 4;
    pub const BUBBLES: u16 = 6;
    pub const CARPET: u16 = 7;
    pub const CONSTRUCTION: u16 = 9;
    pub const CRACKS: u16 = 10;
    pub const CRATERS: u16 = 11;
    pub const CRYSTAL_LATTICE: u16 = 13;
    pub const DENIM: u16 = 14;
    pub const DNA: u16 = 15;
    pub const GRASS: u16 = 17;
    pub const HONEYCOMB: u16 = 20;
    pub const IMPACT: u16 = 21;
    pub const LANDSCAPE: u16 = 22;
    pub const LEAVES: u16 = 23;
    pub const MESH: u16 = 24;
    pub const MOTIFS: u16 = 25;
    pub const PATIO: u16 = 27;
    pub const RECTANGLES: u16 = 28;
    pub const REPTILES: u16 = 29;
    pub const SPIDER_WEB: u16 = 30;
    pub const SPIRALS: u16 = 31;
    pub const SPOKES: u16 = 32;
    pub const STARS: u16 = 35;
    pub const STAR_SHAPES: u16 = 36;
    pub const STONE_WALL: u16 = 37;
    pub const TEXT: u16 = 38;
    pub const TILES: u16 = 39;
    pub const TREE_RINGS: u16 = 40;
    pub const COLOR_BUBBLES: u16 = 43;
    pub const COLOR_CIRCLES: u16 = 44;
    pub const GREEN_GRASS: u16 = 47;
    pub const GREEN_LEAVES: u16 = 49;
    pub const COLOR_LEAVES: u16 = 50;
    pub const COLOR_REPTILES: u16 = 51;
    pub const STAINED_GLASS: u16 = 52;

    /// All the fills drawn per box.
    pub const ALL: [u16; 38] = [
        ARCHIMEDES, BARS, BASKETWEAVE, BIRDS, BUBBLES, CARPET, CONSTRUCTION, CRACKS, CRATERS, CRYSTAL_LATTICE, DENIM, DNA, GRASS,
        HONEYCOMB, IMPACT, LANDSCAPE, LEAVES, MESH, MOTIFS, PATIO, RECTANGLES, REPTILES, SPIDER_WEB, SPIRALS, SPOKES, STARS,
        STAR_SHAPES, STONE_WALL, TEXT, TILES, TREE_RINGS, COLOR_BUBBLES, COLOR_CIRCLES, GREEN_GRASS, GREEN_LEAVES, COLOR_LEAVES,
        COLOR_REPTILES, STAINED_GLASS,
    ];
}

/// Draw fill `index` (its place in the library, 1 is Archimedes) with numbers `p` over a box `w` ×
/// `h` mils at `ppm` pixels per mil. `None` when the fill can't be drawn (a tone stands in).
pub(super) fn draw(index: u16, p: &[i32], w: f64, h: f64, ppm: f64) -> Option<(Vec<u8>, u32, u32)> {
    use fill::*;
    // fills without a seed of their own: their numbers stand in for it
    let seed = p.iter().fold(17u32, |s, v| s.wrapping_mul(31).wrapping_add(*v as u32));
    let mut c = Canvas::new(w, h, ppm, seed)?;
    let a = Args(p);
    let (w, h) = (c.w.round(), c.h.round());
    match index {
        ARCHIMEDES => {
            let (bg, fg, lw, f) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 8, 2.0, 100.0));
            let p = 2000.0 / f;
            background(&mut c, bg);
            c.ink(fg);
            for y in steps(0.0, p, h) {
                for x in steps(0.0, p, w) {
                    c.moveto(x, y);
                    c.show(p * 0.366, (2.732, 0.0), |c| archimedes(c, lw / p * 2.732));
                }
            }
        }
        BARS => bars(&mut c, &a, w, h),
        BASKETWEAVE => {
            let (width, grey, lw, f) = (a.get(3, 100, 1.0, 200.0), a.get(2, 100, 0.0, 100.0), a.get(1, 10, 0.0, 100.0), a.get(0, 6, 1.0, 100.0));
            let d = (width - 100.0) / 100.0;
            let p = 1000.0 / f;
            c.ink(grey);
            c.rows(0.0, w, 0.0, p, h, None, |c| c.show(p * 0.25, (4.0, 0.0), |c| basket_holes(c, d)));
            c.setgray(0.0);
            c.rows(0.0, w, 0.0, p, h, None, |c| c.show(p * 0.25, (4.0, 0.0), |c| basket_weave(c, d, lw / 100.0)));
        }
        BIRDS => {
            let (bg, fg, lw, f) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 4, 0.0, 100.0), a.get(0, 8, 2.0, 100.0));
            let p = 1174.0 / f;
            background(&mut c, bg);
            c.ink(fg);
            c.rows(0.0, w + p, 0.0, p, h + p, None, |c| c.show(p * 0.0061, (138.0, 0.0), |c| birds(c, lw / p * 162.0)));
        }
        BUBBLES | COLOR_BUBBLES | COLOR_CIRCLES => bubbles(&mut c, &a, index, w, h),
        CARPET => carpet(&mut c, &a, w, h),
        CONSTRUCTION => {
            let (bg, fg, lw, f) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 8, 2.0, 100.0));
            let p = 1126.0 / f;
            background(&mut c, bg);
            c.ink(fg);
            c.rows(-p, w, 0.0, p, h, Some((p / 2.1, p)), |c| c.show(p * 0.1, (8.66, 5.0), |c| construction(c, lw / p * 10.0)));
        }
        CRACKS => cracks(&mut c, &a, w, h),
        CRATERS => craters(&mut c, &a, w, h),
        CRYSTAL_LATTICE | HONEYCOMB => layers(&mut c, &a, index, w, h),
        DENIM => denim(&mut c, &a, w, h),
        DNA => {
            let (spacing, bg, fg, lw, f) = (a.get(4, 100, 1.0, 300.0), a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 1, 0.0, 100.0), a.get(0, 4, 1.0, 100.0));
            let p = 2000.0 / f;
            background(&mut c, bg);
            c.ink(fg);
            c.rows(0.0, w, -p, p, h + p, Some((p / 2.1, p)), |c| c.show(p / 360.0, (spacing, 110.0), |c| dna(c, lw * p / 110.0)));
        }
        GRASS | GREEN_GRASS => grass(&mut c, &a, index == GREEN_GRASS, w, h),
        IMPACT => impact(&mut c, &a, w, h),
        LANDSCAPE => landscape(&mut c, &a, w, h),
        LEAVES | GREEN_LEAVES | COLOR_LEAVES => leaves(&mut c, &a, index, w, h),
        MESH => {
            let (fg, sh2, sh1, sq, f) = (a.get(4, 100, 0.0, 100.0), a.get(3, 15, 0.0, 100.0), a.get(2, 3, 0.0, 100.0), a.get(1, 80, 1.0, 100.0), a.get(0, 6, 1.0, 25.0));
            let p = 1000.0 / f;
            c.ink(fg);
            c.rows(0.0, w, 0.0, p, h, None, |c| c.show(p, (1.0, 0.0), |c| mesh(c, sq / 100.0, sh1 / 100.0, 1.0 - sh2 / 100.0)));
        }
        MOTIFS => {
            let (fg, spacing, f, motif) = (a.get(3, 100, 0.0, 100.0), a.get(2, 100, 1.0, 300.0), a.get(1, 2, 1.0, 25.0), a.get(0, 1, 1.0, 8.0) as u8);
            // 8 is a company's logo: not drawn
            if motif == 8 {
                return None;
            }
            let p = 100000.0 / f / spacing;
            let inc = spacing / 100.0 * p;
            c.ink(fg);
            let adv = (spacing * 5.0, spacing * 5.0);
            c.rows(0.0, w, 0.0, inc, h + inc, Some((inc / 2.1, inc)), |c| c.show(p * 0.001, adv, |c| motifs(c, motif)));
        }
        PATIO => {
            let (bg, fg, lw, f) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 8, 2.0, 100.0));
            let p = 1250.0 / f;
            let row = p * 6.0 / 7.0;
            background(&mut c, bg);
            c.ink(fg);
            c.rows(0.0, w, 0.0, row, h, Some((row / 2.0, row)), |c| c.show(p * 0.2857, (2.598, 1.5), |c| patio(c, lw / p * 3.5)));
        }
        RECTANGLES => rectangles(&mut c, &a, w, h),
        REPTILES | COLOR_REPTILES => reptiles(&mut c, &a, index == COLOR_REPTILES, w, h),
        SPIDER_WEB => spider_web(&mut c, &a, w, h),
        SPIRALS => {
            let (bg, fg, lw, size) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 150, 10.0, 500.0));
            background(&mut c, bg);
            let (cx, cy) = (w / 2.0, h / 2.0);
            let half = size / 2.0;
            let hyp = (w * w + h * h).sqrt() / 2.0;
            c.ink(fg);
            c.setlinewidth(lw);
            for r in steps(0.0, size, hyp) {
                c.arc(cx, cy, r, 90.0, 270.0);
                c.arc(cx, cy + half, r + half, -90.0, 90.0);
            }
            c.stroke();
        }
        SPOKES => {
            let (fg, wy, wx, lw, n) = (a.get(4, 100, 0.0, 100.0), a.get(3, 0, 0.0, 100.0), a.get(2, 0, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 120, 4.0, 360.0));
            let len = (w * w + h * h).sqrt();
            c.translate(w * wx / 100.0, h * wy / 100.0);
            for _ in 0..n as usize {
                c.moveto(0.0, 0.0);
                c.lineto(len, 0.0);
                c.rotate(360.0 / n);
            }
            c.ink(fg);
            c.setlinewidth(lw);
            c.stroke();
        }
        STARS => {
            c.srand(a.raw(3, 0));
            let (min, f) = (a.get(2, 3, 1.0, 1000.0), a.get(0, 100, 1.0, 2000.0));
            let max = a.get(1, 300, min, 1000.0);
            c.setgray(0.0);
            c.fill_box();
            c.setgray(1.0);
            let m = c.ctm();
            let ratio = (max / min).trunc();
            for _ in 0..(w * h * f / 100000.0) as usize {
                let (x, y) = (c.rmod(w), c.rmod(h));
                c.moveto(x, y);
                let s = max / (c.rmod(ratio) + 1.0) / 10.0;
                c.scale(s, s);
                c.show(2.0, (1.0, 0.0), |c| {
                    c.moveto(1.0, 0.5);
                    c.arc(0.5, 0.5, 0.5, 0.0, 360.0);
                    c.fill();
                });
                c.set_ctm(m);
                if c.over() {
                    break;
                }
            }
        }
        STAR_SHAPES => {
            let (grey, theta, spacing, f, points) = (a.get(4, 100, 0.0, 100.0), a.get(3, 36, 1.0, 90.0), a.get(2, 100, 1.0, 300.0), a.get(1, 2, 1.0, 25.0), a.get(0, 5, 1.0, 15.0));
            let p = 100000.0 / f / spacing;
            let inc = spacing / 100.0 * p;
            c.ink(grey);
            let adv = (spacing * 5.0, spacing * 5.0);
            c.rows(0.0, w, 0.0, inc, h, Some((inc / 2.1, inc)), |c| c.show(p * 0.001, adv, |c| star(c, points, theta)));
        }
        STONE_WALL | STAINED_GLASS => stone_wall(&mut c, &a, index == STAINED_GLASS, w, h),
        TEXT => text(&mut c, &a, w, h)?,
        TILES => {
            let (bg, fg, lw, f) = (a.get(3, 0, -1.0, 100.0), a.get(2, 100, 0.0, 100.0), a.get(1, 5, 0.0, 100.0), a.get(0, 8, 2.0, 100.0));
            let p = 500.0 / f;
            background(&mut c, bg);
            c.ink(fg);
            c.rows(0.0, w, 0.0, p, h, Some((p / 2.1, p)), |c| c.show(p, (2.0, 0.5), |c| tiles(c, lw / p)));
        }
        TREE_RINGS => {
            c.srand(a.raw(4, 0));
            let (bg, lw, min) = (a.get(3, 0, -1.0, 100.0), a.get(2, 5, 0.0, 100.0), a.get(1, 0, 0.0, 500.0));
            let max = distinct(a.get(0, 150, min, 500.0), min);
            background(&mut c, bg);
            let (cx, cy) = (w / 2.0, h / 2.0);
            let hyp = (w * w + h * h).sqrt();
            c.setgray(0.0);
            c.setlinewidth(lw);
            let mut r = 0.0;
            loop {
                r += c.rmod(max - min) + min;
                c.moveto(cx + r, cy);
                c.arc(cx, cy, r, 0.0, 360.0);
                c.stroke();
                if r > hyp || c.over() {
                    break;
                }
            }
        }
        _ => return None,
    }
    c.finish()
}

/// The fills' "Background gray": a negative one leaves the background clear.
fn background(c: &mut Canvas, gray: f64) {
    if gray >= 0.0 {
        c.ink(gray);
        c.fill_box();
    }
}

fn polyline(c: &mut Canvas, pts: &[(f64, f64)]) {
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 {
            c.moveto(x, y);
        } else {
            c.lineto(x, y);
        }
    }
}

fn polygon(c: &mut Canvas, pts: &[(f64, f64)]) {
    polyline(c, pts);
    c.closepath();
}

/// Archimedes: triangles and squares meeting at every corner (a snub square tiling).
fn archimedes(c: &mut Canvas, lw: f64) {
    polyline(c, &[(0.0, 0.0), (0.5, 0.0), (0.0, 0.866), (0.866, 1.366), (0.0, 1.866), (0.0, 0.866)]);
    polyline(c, &[(0.0, 1.866), (0.5, 2.732), (1.366, 2.232), (0.866, 1.366), (1.866, 1.366), (1.366, 2.232), (2.232, 2.732), (2.732, 1.866), (1.866, 1.366), (2.732, 0.866), (2.232, 0.0), (1.366, 0.5), (1.866, 1.366)]);
    for seg in [[(1.366, 2.732), (1.366, 2.232)], [(1.366, 0.0), (1.366, 0.5)], [(2.232, 0.0), (2.732, 0.0)], [(2.232, 2.732), (2.732, 2.732)], [(0.0, 2.732), (0.5, 2.732)], [(2.732, 0.866), (2.732, 1.866)]] {
        polyline(c, &seg);
    }
    polyline(c, &[(0.5, 0.0), (1.366, 0.5), (0.866, 1.366)]);
    c.setlinewidth(lw);
    c.stroke();
}

/// Bars: shaded bars across and down, darker at their edges, like rods.
fn bars(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    let min = a.get(3, 10, 0.0, 100.0);
    let max = a.get(2, 100, min, 100.0);
    let (spacing, width) = (a.get(1, 100, 0.0, 300.0), a.get(0, 10, 1.0, 100.0));
    let d = max - min;
    let inc = 1.0 + spacing / 100.0;
    let (dx, dy) = (w / width, h / width);
    c.scale(width * 10.0, width * 10.0);
    c.setlinewidth(0.05);
    let rods = |c: &mut Canvas, across: f64, long: f64| {
        let m = c.ctm();
        for t in steps(0.0, inc, across) {
            c.translate(t, 0.0);
            for u in steps(-0.5, 0.05, 0.5) {
                c.moveto(u, 0.0);
                c.lineto(u, long);
                let v = (0.250001 - u * u).max(0.0).sqrt() * 2.0;
                c.setgray(1.0 - (max - v * d) / 100.0);
                c.stroke();
            }
            c.set_ctm(m);
        }
    };
    rods(c, dx, dy);
    c.translate(dx, 0.0);
    c.rotate(90.0);
    rods(c, dy, dx);
}

fn basket_holes(c: &mut Canvas, d: f64) {
    polygon(c, &[(1.0, d), (2.0 - d, 1.0), (1.0, 2.0 - d), (d, 1.0)]);
    c.fill();
    polygon(c, &[(3.0, 2.0 + d), (4.0 - d, 3.0), (3.0, 4.0 - d), (2.0 + d, 3.0)]);
    c.fill();
}

fn basket_weave(c: &mut Canvas, d: f64, lw: f64) {
    for seg in [
        [(0.0, 3.0 + d), (1.0 - d, 4.0)],
        [(0.0, 1.0 + d), (1.0, d)],
        [(3.0 - d, 4.0), (4.0 - d, 3.0)],
        [(1.0 - d, 0.0), (2.0 - d, 1.0)],
        [(4.0, 3.0 + d), (3.0, 2.0 + d)],
        [(3.0 - d, 0.0), (1.0, 2.0 - d)],
        [(4.0, 1.0 + d), (2.0 + d, 3.0)],
        [(d, 1.0), (3.0, 4.0 - d)],
    ] {
        polyline(c, &seg);
    }
    c.setlinewidth(lw);
    c.stroke();
}

/// Birds: three flying birds, mirrored in turn, twice.
fn birds(c: &mut Canvas, lw: f64) {
    for _ in 0..2 {
        c.gsave();
        for _ in 0..3 {
            c.moveto(-10.0, -8.0);
            c.curveto(60.0, 24.0, -54.0, 60.0, -9.0, 72.0);
            c.curveto(-2.5, 73.7, 11.5, 70.3, 29.0, 75.4);
            c.moveto(-54.0, 6.0);
            c.curveto(-45.0, 14.0, -27.0, 16.0, -18.0, 18.0);
            c.curveto(27.0, 27.0, -81.0, 54.0, -9.0, 90.0);
            c.moveto(-126.0, 9.0);
            c.curveto(-114.0, 27.0, -66.0, 66.0, -54.0, 24.0);
            c.curveto(-53.0, 21.0, -49.0, 15.0, -43.0, 12.0);
            c.concat([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            c.translate(135.0, -81.0);
        }
        c.setlinewidth(lw);
        c.stroke();
        c.grestore();
        c.translate(138.0, 0.0);
    }
}

/// Bubbles (white, outlined in black), ColorBubbles (outlined in random colors) and
/// ColorCircles (filled with random colors), scattered at random.
fn bubbles(c: &mut Canvas, a: &Args, index: u16, w: f64, h: f64) {
    let circles = index == fill::COLOR_CIRCLES;
    let (min, n, lw) = if circles {
        c.srand(a.raw(3, 0));
        (a.get(2, 10, 1.0, 1000.0), a.get(0, 25, 1.0, 250.0), 0.0)
    } else {
        c.srand(a.raw(4, 0));
        (a.get(2, 10, 1.0, 1000.0), a.get(0, 25, 1.0, 250.0), a.get(3, 10, 0.0, 50.0))
    };
    let max = a.get(1, 300, min, 1000.0);
    let size = (max / min).trunc();
    for _ in 0..(w * h * n / 1e6) as usize {
        let (x, y) = (c.rmod(w), c.rmod(h));
        let r = size / (c.rmod(size) + 1.0) * min;
        c.moveto(x + r, y);
        c.arc(x, y, r, 0.0, 360.0);
        if circles {
            c.random_rgb();
            c.fill();
        } else {
            c.gsave();
            if index == fill::COLOR_BUBBLES {
                c.random_rgb();
            } else {
                c.setgray(0.0);
            }
            c.setlinewidth(lw);
            c.stroke();
            c.grestore();
            c.setgray(1.0);
            c.fill();
        }
        if c.over() {
            break;
        }
    }
}

/// Carpet: a woven-looking grid of tiny squares where two sine waves meet.
fn carpet(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    let (alpha, modf, gamma) = (a.raw(4, 10) as f64, a.raw(3, 3).max(1), a.raw(2, 50).max(1));
    let (grey, f) = (a.get(1, 100, 0.0, 100.0), a.get(0, 72, 10.0, 300.0));
    let mut wz: i64 = 360;
    for k in 2..=((gamma as f64).sqrt() as i64) {
        if gamma % k == 0 && wz % k == 0 {
            wz /= k;
        }
    }
    let wzf = wz as f64;
    let g = gamma as f64;
    let mut cells = vec![];
    for i in 0..=wz {
        for j in 0..=wz {
            let v = ((i as f64 * g - 15.0).to_radians().sin() + (j as f64 * g - 10.0).to_radians().sin()) * alpha;
            if (v.trunc() as i64) % modf == 0 {
                cells.push((j as f64, i as f64));
            }
        }
    }
    let p = wzf * 1000.0 / f;
    c.ink(grey);
    c.rows(0.0, w, 0.0, p, h, Some((p / 2.0, p)), |c| {
        c.show(p / wzf, (wzf, 0.0), |c| {
            for &(x, y) in &cells {
                polygon(c, &[(x, y), (x + 1.0, y), (x + 1.0, y + 1.0), (x, y + 1.0)]);
            }
            c.fill();
        })
    });
}

/// Construction: rows of rounded triangle-and-diamond frames.
fn construction(c: &mut Canvas, lw: f64) {
    c.moveto(1.0, 0.0);
    c.arc(0.0, 0.0, 1.0, -60.0, 300.0);
    c.lineto(9.1602, 4.1339);
    c.arc(8.6602, 5.0, 1.0, -60.0, 420.0);
    c.lineto(0.5, 10.866);
    c.arc(0.0, 10.0, 1.0, 60.0, 180.0);
    c.lineto(-1.0, 0.0);
    polyline(c, &[(-0.5, 3f64.sqrt() / 2.0), (8.1602, 5.866)]);
    polyline(c, &[(8.1602, 4.1339), (-0.5, 9.134)]);
    polyline(c, &[(1.0, 10.0), (1.0, 0.0)]);
    c.setlinewidth(lw);
    c.stroke();
}

/// Cracks: random walks of short steps.
fn cracks(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    let (lw, step, min) = (a.get(4, 5, 0.0, 100.0), a.get(3, 14, 1.0, 100.0), a.get(2, 75, 1.0, 300.0));
    let max = distinct(a.get(1, 125, min, 300.0), min);
    let n = a.get(0, 20, 1.0, 100.0);
    for _ in 0..n as usize {
        c.gsave();
        let theta = c.rmod(360.0);
        let (x, y) = (c.rmod(w), c.rmod(h));
        c.moveto(x, y);
        c.scale(step, step);
        c.setlinewidth(lw / step);
        let len = c.rmod(max - min) + min;
        for _ in 0..len as usize {
            let (x, y) = c.currentpoint();
            c.translate(x, y);
            let t = c.rmod(120.0) - 60.0 + theta;
            c.rotate(t);
            c.moveto(0.0, 0.0);
            c.lineto(1.0, 0.0);
            c.stroke();
            c.moveto(1.0, 0.0);
            c.rotate(-t);
        }
        c.grestore();
        if c.over() {
            break;
        }
    }
}

/// Craters: circles of the background gray with rays out of them.
fn craters(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let (bg, min) = (a.get(3, 0, 0.0, 100.0), a.get(2, 75, 1.0, 500.0));
    let max = distinct(a.get(1, 300, min, 500.0), min);
    let n = a.get(0, 15, 1.0, 50.0);
    c.ink(bg);
    c.fill_box();
    let dif = (max - min).trunc();
    let m = c.ctm();
    for _ in 0..(w * h / 1e6 * n) as usize {
        let (x, y) = (c.rmod(w), c.rmod(h));
        c.translate(x, y);
        let size = c.rmod(dif) + min;
        c.arc(0.0, 0.0, size * 0.7, 0.0, 360.0);
        c.ink(bg);
        c.fill();
        let mut t = 0.0;
        loop {
            t += c.rmod(18.0) + 10.0;
            if t > 360.0 {
                break;
            }
            c.rotate(t);
            c.moveto(size / 5.0, 0.0);
            let r = (c.rmod(300.0) + 200.0) / 500.0 * size;
            c.lineto(r, 0.0);
            c.rotate(-t);
        }
        c.setgray(0.0);
        c.setlinewidth(5.0);
        c.stroke();
        c.set_ctm(m);
    }
}

/// CrystalLattice (squares with dots at their corners) and Honeycomb (hexagons): ten layers of
/// the grid from the middle of the box, each larger and a step from the back gray to the front.
fn layers(c: &mut Canvas, a: &Args, index: u16, w: f64, h: f64) {
    let honey = index == fill::HONEYCOMB;
    let lw = if honey { a.get(4, 5, 0.0, 100.0) } else { 0.0 };
    let (scaling, front, back, f) = (a.get(3, 75, 10.0, 100.0), a.get(2, 0, 0.0, 100.0), a.get(1, 100, -100.0, 100.0), a.get(0, 4, 1.0, 50.0));
    let p = (1000.0 / f).trunc();
    c.translate(w / 2.0, h / 2.0);
    let (dx, dy) = (w * 100.0 / scaling, h * 100.0 / scaling);
    c.scale(scaling / 100.0, scaling / 100.0);
    let grow = 10f64.powf((100.0 / scaling).log10() / 10.0);
    let start = |d: f64| {
        let a = (d / 2.0).trunc();
        -(a - (a % p - p / 2.0))
    };
    for g in steps(back.clamp(0.0, 100.0) / 100.0, (front - back) / 1000.0, (front - 0.1) / 100.0) {
        c.setgray(1.0 - g);
        c.scale(grow, grow);
        let x0 = start(dx);
        c.rows(x0, (dx + p) / 2.0, start(dy), p, (dy + p) / 2.0, Some((p / 2.1, p)), |c| {
            if honey {
                c.show(p * 0.5773, (1.5, 0.866), |c| {
                    polygon(c, &[(0.5, 0.0), (1.5, 0.0), (2.0, 0.866), (1.5, 1.732), (0.5, 1.732), (0.0, 0.866)]);
                    c.setlinewidth(lw / p * 3f64.sqrt());
                    c.stroke();
                })
            } else {
                c.show(p, (1.0, 0.0), crystal)
            }
        });
        if c.over() {
            break;
        }
    }
}

fn crystal(c: &mut Canvas) {
    c.gsave();
    c.moveto(0.0, 0.0);
    for _ in 0..3 {
        c.lineto(1.0, 0.0);
        let (x, y) = c.currentpoint();
        c.translate(x, y);
        c.rotate(90.0);
    }
    c.closepath();
    c.setlinewidth(0.05);
    c.stroke();
    c.grestore();
    c.gsave();
    for _ in 0..4 {
        c.moveto(0.2, 0.0);
        c.arc(0.0, 0.0, 0.2, 0.0, 360.0);
        c.fill();
        c.translate(1.0, 0.0);
        c.rotate(90.0);
    }
    c.grestore();
}

/// Denim: a fine image of random grays.
fn denim(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let min = a.get(2, 0, 0.0, 100.0);
    let max = a.get(1, 100, min, 100.0);
    let f = a.get(0, 72, 1.0, 300.0) / 1000.0;
    let d = (max - min) / 100.0;
    let (cols, rows) = ((w * f) as u32 + 1, (h * f) as u32 + 1);
    if (cols as u64) * (rows as u64) > BUDGET as u64 {
        c.spend(BUDGET);
        return;
    }
    let Some(mut img) = sk::Pixmap::new(cols, rows) else { return };
    {
        let px = img.data_mut();
        // the image's first row is at the bottom
        for j in 0..rows {
            for i in 0..cols {
                let v = (((c.rand() >> 11) & 255) as f64 * d + min * 2.55) as u8;
                let o = (((rows - 1 - j) * cols + i) * 4) as usize;
                px[o..o + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
    }
    let m = c.ctm() * Affine::translate((0.0, rows as f64 / f)) * Affine::scale_non_uniform(1.0 / f, -1.0 / f);
    let [sa, sb, sc, sd, se, sf] = m.as_coeffs();
    let t = sk::Transform::from_row(sa as f32, sb as f32, sc as f32, sd as f32, se as f32, sf as f32);
    let paint = sk::PixmapPaint { quality: sk::FilterQuality::Nearest, ..Default::default() };
    c.flush();
    c.pm.draw_pixmap(0, 0, img.as_ref(), &paint, t, None);
}

/// DNA: double helixes with rungs, a row climbing as it goes.
fn dna(c: &mut Canvas, lw: f64) {
    c.setlinewidth(lw);
    c.moveto(0.0, 0.0);
    for i in 0..=360 {
        let a = (i as f64).to_radians();
        c.lineto(a.sin() * 20.0, i as f64);
    }
    c.stroke();
    c.moveto(20.0, 0.0);
    for i in 0..=360 {
        let a = (i as f64).to_radians();
        c.lineto(a.cos() * 20.0, i as f64);
    }
    c.stroke();
    for i in (0..=360).step_by(20) {
        let a = (i as f64).to_radians();
        c.moveto(a.sin() * 20.0, i as f64);
        c.lineto(a.cos() * 20.0, i as f64);
    }
    c.stroke();
}

/// Grass (white blades) and GreenGrass (blades of random greens), scattered.
fn grass(c: &mut Canvas, a: &Args, green: bool, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let (grey, min) = (a.get(3, 0, -1.0, 100.0), a.get(2, 7, 1.0, 100.0));
    let max = distinct(a.get(1, 35, min, 100.0), min);
    let n = a.get(0, 100, 1.0, 500.0);
    background(c, grey);
    let (llx, lly) = (-max, -max);
    let (dx, dy) = (w - llx, h - lly);
    let ds = max - min;
    for _ in 0..(dx * dy / 1e6 * n) as usize {
        let m = c.ctm();
        let (x, y) = (c.rmod(dx) + llx, c.rmod(dy) + lly);
        c.translate(x, y);
        let s = c.rmod(ds) + min;
        c.scale(s, s);
        c.moveto(-0.5, 0.0);
        let r = c.rmod(14.0) - 7.0;
        c.curveto(-0.5, 3.0, r / 3.0 - 0.3, 10.0, r, 10.0);
        c.curveto(r / 3.0 + 0.3, 10.0, 0.5, 3.0, 0.5, 0.0);
        if green {
            c.closepath();
        }
        c.gsave();
        if green {
            let g = (1.0 - (c.rmod(100.0) + 1.0) / 100.0).max(0.7);
            c.setrgb(0.0, g, 0.0);
        } else {
            c.setgray(1.0);
        }
        c.fill();
        c.grestore();
        c.setlinewidth(0.1);
        c.setgray(0.0);
        c.stroke();
        c.set_ctm(m);
        if c.over() {
            break;
        }
    }
}

/// Impact: random walks out of the middle of the box, like cracks from a blow.
fn impact(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let min = a.get(3, 10, 2.0, 90.0);
    let max = distinct(a.get(2, 40, min, 90.0), min);
    let (step, lw) = (a.get(1, 15, 10.0, 500.0), a.get(0, 5, 0.0, 100.0));
    c.translate(w / 2.0, h / 2.0);
    c.setlinewidth(lw / step);
    c.scale(step, step);
    let mut theta = 0.0;
    loop {
        let m = c.ctm();
        theta += c.rmod(max - min) + min;
        if theta > 360.0 || c.over() {
            break;
        }
        c.rotate(theta);
        c.moveto(0.0, 0.0);
        for _ in 0..(c.rmod(150.0) + 50.0) as usize {
            let (x, y) = c.currentpoint();
            c.translate(x, y);
            let t = c.rmod(120.0) - 60.0 + theta;
            c.rotate(t);
            c.lineto(1.0, 0.0);
            c.rotate(-t);
        }
        c.stroke();
        c.set_ctm(m);
    }
}

/// Landscape: a fractal terrain of shaded facets seen from above at an angle.
fn landscape(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(3, 0));
    let min = a.get(2, 0, 0.0, 100.0);
    let max = a.get(1, 100, min, 100.0);
    let depth = a.get(0, 6, 1.0, 7.0) as u32;
    let size = (1usize << (depth - 1)) + 1;
    let mut land = Land { grid: vec![0.0; size * size], size, max_depth: depth, depth: 0, horz: 0, vert: 0, height: 1.8f64.powi(8 - depth as i32), d: (max - min) / 200.0, mid: (max + min) / 200.0 };
    let hyp = (w * w + h * h).sqrt();
    c.translate(w / 2.0, h / 2.0);
    c.scale(hyp * 1.2, hyp * 1.2);
    c.rotate(45.0);
    c.translate(-0.5, -0.5);
    land.square(c);
}

struct Land {
    grid: Vec<f64>,
    size: usize,
    max_depth: u32,
    depth: u32,
    horz: usize,
    vert: usize,
    height: f64,
    d: f64,
    mid: f64,
}

impl Land {
    fn at(&self, h: usize, v: usize) -> f64 {
        self.grid[h * self.size + v]
    }
    fn set(&mut self, h: usize, v: usize, x: f64) {
        self.grid[h * self.size + v] = x;
    }
    fn square(&mut self, c: &mut Canvas) {
        self.depth += 1;
        let (h, v) = (self.horz, self.vert);
        if self.depth >= self.max_depth {
            let (a, b, cc, d) = (self.at(h, v), self.at(h + 1, v), self.at(h + 1, v + 1), self.at(h, v + 1));
            polygon(c, &[(a + 1.0, a + 1.0), (b, b + 1.0), (cc, cc), (d + 1.0, d)]);
            c.setgray((a - cc) * self.d + self.mid);
            c.fill();
        } else {
            let wd = 1usize << (self.max_depth - self.depth - 1);
            let (e, f, g, k) = (self.at(h + 2 * wd, v + 2 * wd), self.at(h, v + 2 * wd), self.at(h + 2 * wd, v), self.at(h, v));
            let scale = self.height * 2f64.powi(self.depth as i32);
            let shift = |c: &mut Canvas| (c.rmod(50.0) - 25.0) / scale;
            let x = (e + f + g + k) / 4.0 + shift(c);
            self.set(h + wd, v + wd, x);
            let x = (e + g) / 2.0 + shift(c);
            self.set(h + 2 * wd, v + wd, x);
            let x = (e + f) / 2.0 + shift(c);
            self.set(h + wd, v + 2 * wd, x);
            if h == 0 {
                let x = (f + k) / 2.0 + shift(c);
                self.set(h, v + wd, x);
            }
            if v == 0 {
                let x = (g + k) / 2.0 + shift(c);
                self.set(h + wd, v, x);
            }
            for (tx, ty, dh, dv) in [(0.5, 0.5, 0, 0), (-0.5, 0.0, wd, 0), (0.5, -0.5, 0, wd), (-0.5, 0.0, wd, wd)] {
                c.translate(tx, ty);
                c.scale(0.5, 0.5);
                self.horz = h + dh;
                self.vert = v + dv;
                self.square(c);
                c.scale(2.0, 2.0);
            }
            self.horz = h;
            self.vert = v;
        }
        self.depth -= 1;
    }
}

/// Leaves (random grays), GreenLeaves (random greens) and ColorLeaves (random colors), scattered
/// at random angles.
fn leaves(c: &mut Canvas, a: &Args, index: u16, w: f64, h: f64) {
    let plain = index == fill::LEAVES;
    let green = index == fill::GREEN_LEAVES;
    let (min, max, lo, hi, n) = if index == fill::COLOR_LEAVES {
        let min = a.get(2, 10, 1.0, 200.0);
        (min, distinct(a.get(1, 100, min, 200.0), min), 0.0, 0.0, a.get(0, 50, 1.0, 250.0))
    } else {
        let min = a.get(4, 10, 1.0, 200.0);
        let max = distinct(a.get(3, 100, min, 200.0), min);
        let lo = a.get(2, if green { 70 } else { 0 }, 0.0, 100.0);
        (min, max, lo, a.get(1, 100, lo, 100.0), a.get(0, 50, 1.0, 250.0))
    };
    for _ in 0..(w * h * n / 1e6) as usize {
        let m = c.ctm();
        let (x, y) = (c.rmod(w), c.rmod(h));
        c.translate(x, y);
        let r = c.rmod(360.0);
        c.rotate(r);
        let s = (c.rmod(max - min) + min) / 10.8;
        c.scale(s, s);
        c.moveto(17.0, 0.0);
        c.curveto(65.0, -18.0, 106.0, -13.0, 125.0, 0.0);
        c.curveto(106.0, 13.0, 65.0, 18.0, 17.0, 0.0);
        c.gsave();
        if plain || green {
            let g = if hi == lo { hi / 100.0 } else { (c.rmod(hi - lo) + lo) / 100.0 };
            if green {
                c.setrgb(0.0, g, 0.0);
            } else {
                c.setgray(g);
            }
        } else {
            c.random_rgb();
        }
        c.fill();
        c.grestore();
        c.setlinewidth(0.3);
        c.setgray(0.0);
        c.stroke();
        c.set_ctm(m);
        if c.over() {
            break;
        }
    }
}

/// Mesh: square frames, their opening off center as if lit from a corner.
fn mesh(c: &mut Canvas, size: f64, lo: f64, hi: f64) {
    c.scale(size, size);
    polygon(c, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
    polygon(c, &[(lo, lo), (lo, hi), (hi, hi), (hi, lo)]);
    c.fill();
}

/// The Motifs fill's pictures, in a 500 unit cell.
fn motifs(c: &mut Canvas, motif: u8) {
    match motif {
        // a maple leaf flag
        1 => {
            c.scale(9.6, 9.6);
            c.translate(9.0, -30.0);
            polygon(c, &[(-9.0, 60.0), (-9.0, 30.0), (-1.0, 30.0), (-1.0, 60.0)]);
            polygon(c, &[(43.0, 60.0), (43.0, 30.0), (35.0, 30.0), (35.0, 60.0)]);
            polygon(
                c,
                &[
                    (17.0, 58.0), (15.0, 54.0), (12.0, 55.0), (14.0, 47.0), (10.0, 51.0), (10.0, 49.0), (5.0, 50.0), (7.0, 45.0), (5.0, 45.0),
                    (12.0, 39.0), (10.0, 37.0), (16.5, 38.0), (16.5, 32.0), (17.5, 32.0), (17.5, 38.0), (24.0, 37.0), (22.0, 39.0), (29.0, 45.0),
                    (27.0, 45.0), (29.0, 50.0), (24.0, 49.0), (24.0, 51.0), (20.0, 47.0), (22.0, 55.0), (19.0, 54.0),
                ],
            );
            c.fill();
        }
        // seven squares around a circle
        2 => {
            c.translate(250.0, 250.0);
            c.scale(113.0, 113.0);
            for _ in 0..7 {
                c.rotate(45.0);
                c.gsave();
                c.translate(1.7071, 0.0);
                polygon(c, &[(0.5, 0.5), (-0.5, 0.5), (-0.5, -0.5), (0.5, -0.5)]);
                c.fill();
                c.grestore();
            }
        }
        // a globe: meridians and parallels
        3 => {
            c.translate(250.0, 250.0);
            c.scale(250.0, 250.0);
            for i in 0..=4 {
                let m = c.ctm();
                c.scale((22.5 * i as f64).to_radians().sin(), 1.0);
                c.arc(0.0, 0.0, 1.0, 90.0, 450.0);
                c.set_ctm(m);
            }
            for i in -3..=3 {
                let s = (22.5 * i as f64).to_radians().sin();
                let r = (1.0 - s * s).sqrt();
                c.moveto(-r, s);
                c.lineto(r, s);
            }
            c.setlinewidth(0.01);
            c.stroke();
        }
        // a cube, seen from a corner: its visible edges, or all of them
        4 | 5 => {
            c.translate(250.0, 250.0);
            c.scale(145.0, 145.0);
            let (s, k) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
            let at = |x: f64, y: f64, z: f64| {
                let (y1, z1) = (y * k - z * s, y * s + z * k);
                (x * k - z1 * s, y1)
            };
            let line = |c: &mut Canvas, pts: &[(f64, f64, f64)]| {
                let pts: Vec<_> = pts.iter().map(|&(x, y, z)| at(x, y, z)).collect();
                polyline(c, &pts);
            };
            line(c, &[(1.0, 1.0, 1.0), (-1.0, 1.0, 1.0), (-1.0, -1.0, 1.0), (1.0, -1.0, 1.0)]);
            c.closepath();
            if motif == 4 {
                line(c, &[(-1.0, 1.0, 1.0), (-1.0, 1.0, -1.0), (1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0)]);
                line(c, &[(1.0, 1.0, 1.0), (1.0, 1.0, -1.0)]);
            } else {
                line(c, &[(1.0, 1.0, -1.0), (-1.0, 1.0, -1.0), (-1.0, -1.0, -1.0), (1.0, -1.0, -1.0)]);
                c.closepath();
                for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                    line(c, &[(x, y, 1.0), (x, y, -1.0)]);
                }
            }
            c.setlinewidth(0.01);
            c.stroke();
        }
        // a ball with a crescent highlight
        6 => {
            c.translate(250.0, 250.0);
            c.scale(225.0, 225.0);
            c.arc(0.0, 0.0, 1.1, 0.0, 360.0);
            c.translate(-0.32, 0.55);
            c.rotate(30.0);
            c.scale(0.5, 0.3333);
            c.arcn(0.0, 0.0, 1.1, 360.0, 0.0);
            c.fill();
        }
        // a checkerboard
        _ => {
            polygon(c, &[(0.0, 0.0), (500.0, 0.0), (500.0, 500.0), (0.0, 500.0)]);
            c.fill();
        }
    }
}

/// Patio: three-armed tiles locking into each other.
fn patio(c: &mut Canvas, lw: f64) {
    c.translate(1.732, 1.5);
    c.moveto(0.866, 0.5);
    for _ in 0..3 {
        c.rotate(120.0);
        for (x, y) in [(0.866, -1.5), (1.732, -1.0), (1.732, 0.0), (0.866, 0.5)] {
            c.lineto(x, y);
        }
    }
    c.setlinewidth(lw);
    c.stroke();
}

/// Rectangles: rectangles of random shapes, all of the same area.
fn rectangles(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let (grey, lw, n, area) = (a.get(3, 0, 0.0, 100.0), a.get(2, 5, 0.0, 100.0), a.get(1, 50, 1.0, 200.0), a.get(0, 100, 10.0, 300.0) * 10000.0);
    let (dx, dy) = (w * 2.0, h * 2.0);
    c.setlinewidth(lw);
    c.translate(-w, -h);
    for _ in 0..n as usize {
        let (x, y) = (c.rmod(dx), c.rmod(dy));
        c.moveto(x, y);
        let (r1, r2) = (c.rmod(area), c.rmod(area));
        let s = (r1 * r2).sqrt().sqrt();
        if s > 0.0 {
            c.rlineto(s, 0.0);
            c.rlineto(0.0, area / s);
            c.rlineto(-s, 0.0);
            c.closepath();
        }
        c.gsave();
        c.ink(grey);
        c.fill();
        c.grestore();
        c.setgray(0.0);
        c.stroke();
    }
}

/// Reptiles (three grays) and ColorReptiles (random colors): interlocking lizards in threes,
/// outlined.
fn reptiles(c: &mut Canvas, a: &Args, color: bool, w: f64, h: f64) {
    let (lw, grays, f) = if color {
        (a.get(1, 8, 0.0, 250.0), [0.0; 3], a.get(0, 4, 1.0, 100.0))
    } else {
        let g = [a.get(1, 60, -1.0, 100.0) / 100.0, a.get(2, 30, -1.0, 100.0) / 100.0, a.get(3, 0, 0.0, 100.0) / 100.0];
        (a.get(4, 8, 0.0, 250.0), g, a.get(0, 4, 1.0, 100.0))
    };
    let size = 2000.0 / f;
    let k = size * 0.2857;
    let step = size * 6.0 / 7.0;
    let rise = size * 2.0 / 7.0 * 0.49;
    let adv = (2.598, 1.5);
    for y in steps(0.0, step, h + step + rise) {
        c.moveto(0.0, y);
        loop {
            let (x, yy) = c.currentpoint();
            for (turn, g) in [(240.0, grays[0]), (120.0, grays[1])] {
                c.translate(x, yy);
                c.rotate(turn);
                if color {
                    c.random_rgb();
                    c.show(k, adv, reptile);
                } else if g >= 0.0 {
                    c.setgray(1.0 - g);
                    c.show(k, adv, reptile);
                }
                c.moveto(0.0, 0.0);
                c.rotate(-turn);
                c.translate(-x, -yy);
            }
            if color {
                c.random_rgb();
            } else {
                c.setgray(1.0 - grays[2]);
            }
            c.show(k, adv, reptile);
            let (cx, mut cy) = c.currentpoint();
            if cy - y > step / 2.1 {
                cy -= step;
            }
            if x > w || c.over() {
                break;
            }
            c.moveto(cx, cy);
        }
    }
    if lw > 0.0 {
        c.setgray(0.0);
        c.rows(0.0, w, 0.0, step, h + step, Some((step / 2.1, step)), |c| c.show(k, adv, |c| reptile_lines(c, lw / size * 3.5)));
    }
}

const REPTILE: [(f64, f64); 50] = [
    (0.0, 0.0), (0.32, -0.40), (0.32, -0.48), (0.0, -0.72), (-0.40, -0.55), (-0.47, -0.68), (-0.42, -0.97), (-0.27, -0.99), (-0.21, -0.88),
    (0.05, -1.03), (0.4, -0.76), (0.84, -0.84), (0.5, -0.96), (0.31, -1.18), (0.32, -1.39), (0.55, -1.60), (0.59, -1.74), (0.82, -1.86),
    (0.87, -1.5), (0.58, -1.28), (0.8, -1.14), (0.94, -1.18), (1.24, -1.08), (1.42, -1.18), (1.52, -1.45), (1.45, -1.81), (1.74, -1.47),
    (1.68, -1.02), (1.52, -0.84), (1.64, -0.66), (1.73, -0.36), (2.28, -0.46), (2.36, -0.11), (2.12, -0.15), (1.73, 0.0), (1.41, -0.26),
    (1.32, -0.49), (1.06, -0.24), (1.42, 0.18), (1.21, 0.41), (1.11, 0.60), (0.87, 0.57), (0.87, 0.26), (0.99, 0.26), (1.05, 0.12),
    (0.82, -0.07), (0.68, -0.07), (0.62, 0.36), (0.26, 0.52), (0.19, 0.48),
];

fn reptile(c: &mut Canvas) {
    polygon(c, &REPTILE);
    c.fill();
}

fn reptile_lines(c: &mut Canvas, lw: f64) {
    let lines: [&[(f64, f64)]; 6] = [
        &[(0.0, 0.0), (0.32, -0.40), (0.32, -0.48), (0.0, -0.72)],
        &[(0.05, -1.03), (0.4, -0.76), (0.84, -0.84), (0.5, -0.96), (0.31, -1.18)],
        &[(0.87, -1.5), (0.58, -1.28), (0.8, -1.14), (0.94, -1.18), (1.24, -1.08), (1.42, -1.18)],
        &[(1.68, -1.02), (1.52, -0.84), (1.64, -0.66), (1.73, -0.36)],
        &[(1.73, 0.0), (1.41, -0.26), (1.32, -0.49), (1.06, -0.24), (1.42, 0.18)],
        &[(0.87, 0.57), (0.87, 0.26), (0.99, 0.26), (1.05, 0.12), (0.82, -0.07), (0.68, -0.07), (0.62, 0.36)],
    ];
    c.moveto(0.866, 0.5);
    for _ in 0..3 {
        c.rotate(120.0);
        for l in lines {
            polyline(c, l);
        }
        c.moveto(0.866, 0.5);
    }
    c.setlinewidth(lw);
    c.stroke();
}

/// SpiderWeb: threads out of the middle at random angles, joined by arcs.
fn spider_web(c: &mut Canvas, a: &Args, w: f64, h: f64) {
    c.srand(a.raw(4, 0));
    let min = a.get(3, 10, 2.0, 90.0);
    let max = distinct(a.get(2, 40, min, 90.0), min);
    let (sep, lw) = (a.get(1, 300, 10.0, 500.0), a.get(0, 5, 0.0, 100.0));
    let hyp = (w * w + h * h).sqrt();
    c.translate(w / 2.0, h / 2.0);
    let (mut theta, mut dtheta) = (0.0f64, 0.0f64);
    loop {
        c.moveto(0.0, 0.0);
        theta += dtheta;
        if theta >= 360.0 || c.over() {
            break;
        }
        dtheta = c.rmod(max - min) + min;
        if theta + dtheta > 350.0 {
            dtheta = 360.0 - theta;
        }
        let (t, t2) = (theta.to_radians(), (theta + dtheta).to_radians());
        c.lineto(hyp * t.cos(), hyp * t.sin());
        for r in steps(0.0, sep, hyp) {
            c.moveto(r * t.cos(), r * t.sin());
            c.arcn(r * (t2.cos() + t.cos()), r * (t2.sin() + t.sin()), r, theta + 180.0 + dtheta, theta + 180.0);
        }
        c.setlinewidth(lw);
        c.stroke();
    }
}

/// StarShapes: a star of `points` points, `theta` degrees at each point.
fn star(c: &mut Canvas, points: f64, theta: f64) {
    c.translate(250.0, 250.0);
    c.scale(250.0, 250.0);
    c.rotate(90.0);
    let t1 = (180.0 / points).to_radians().tan();
    let t2 = (theta / 2.0).to_radians().tan();
    c.moveto(1.0, 0.0);
    for _ in 0..points as usize {
        c.rotate(360.0 / points);
        let x = t2 / (t1 + t2);
        c.lineto(x, -x * t1);
        c.lineto(1.0, 0.0);
    }
    c.closepath();
    c.fill();
}

/// StoneWall (stones of random grays) and StainedGlass (panes of random colors): rows of
/// irregular stones, the same for every drawing (the procedure seeds its own random numbers).
fn stone_wall(c: &mut Canvas, a: &Args, glass: bool, w: f64, h: f64) {
    let (lw, lo, hi, f) = if glass {
        (a.get(1, 5, 0.0, 100.0), 0.0, 0.0, a.get(0, 15, 1.0, 50.0))
    } else {
        let lo = a.get(2, 0, 0.0, 100.0);
        (a.get(3, 5, 0.0, 100.0), lo, a.get(1, 100, lo, 100.0), a.get(0, 15, 1.0, 50.0))
    };
    let dif = if hi - lo == 0.0 { 1.0 } else { hi - lo };
    c.setlinewidth(lw * f / 250.0);
    c.srand(0);
    c.translate(0.0, h);
    c.scale(250.0 / f, 250.0 / f);
    for _ in 0..(h / 920.0 * f) as usize {
        c.moveto(0.0, 0.0);
        let mut q = [(0.0, 0.0); 4];
        for k in steps(0.0, 5.0, w / 200.0 * f) {
            let v = k + c.rmod(50.0) / 25.0 - 1.0;
            c.moveto(q[3].0, q[3].1);
            c.curveto(q[2].0, q[2].1, q[1].0, q[1].1, q[0].0, q[0].1);
            q[0] = (v, -c.rmod(30.0) / 15.0 - 2.0);
            c.lineto(q[0].0, q[0].1);
            let x1 = v + c.rmod(50.0) / 10.0 - 2.5;
            q[1] = (x1, -c.rmod(50.0) / 10.0);
            q[2] = (x1, c.rmod(50.0) / 10.0);
            q[3] = (v, c.rmod(30.0) / 15.0 + 2.0);
            c.curveto(q[1].0, q[1].1, q[2].0, q[2].1, q[3].0, q[3].1);
            c.closepath();
            c.gsave();
            if glass {
                c.random_rgb();
            } else {
                let g = (c.rmod(dif) + lo) / 100.0;
                c.setgray(1.0 - g);
            }
            c.fill();
            c.grestore();
            c.setgray(0.0);
            c.stroke();
        }
        c.translate(0.0, -4.0);
        if c.over() {
            break;
        }
    }
}

/// Text: rows of one character of one of the standard PostScript fonts (or the closest font
/// installed), black.
fn text(c: &mut Canvas, a: &Args, w: f64, h: f64) -> Option<()> {
    let (bg, spacing, f, ch, font) = (a.get(4, 0, -1.0, 100.0), a.get(3, 100, 30.0, 300.0), a.get(2, 4, 1.0, 50.0), a.get(1, 67, 33.0, 255.0), a.get(0, 1, 1.0, 35.0) as u8);
    let (glyph, advance) = glyph(font, super::super::text::cp1252(ch as u8))?;
    let p = 100000.0 / f / spacing;
    let inc = spacing / 100.0 * p * 2.0;
    background(c, bg);
    c.setgray(0.0);
    for y in steps(0.0, inc, h + p) {
        c.moveto(0.0, y);
        loop {
            c.show(p, (advance, 0.0), |c| {
                for el in glyph.elements() {
                    match *el {
                        PathEl::MoveTo(q) => c.moveto(q.x, q.y),
                        PathEl::LineTo(q) => c.lineto(q.x, q.y),
                        PathEl::QuadTo(q1, q2) => {
                            let (x0, y0) = c.currentpoint();
                            let p0 = Point::new(x0, y0);
                            let a1 = p0 + (q1 - p0) * (2.0 / 3.0);
                            let a2 = q2 + (q1 - q2) * (2.0 / 3.0);
                            c.curveto(a1.x, a1.y, a2.x, a2.y, q2.x, q2.y);
                        }
                        PathEl::CurveTo(q1, q2, q3) => c.curveto(q1.x, q1.y, q2.x, q2.y, q3.x, q3.y),
                        PathEl::ClosePath => c.closepath(),
                    }
                }
                c.fill();
            });
            let (cx, mut cy) = c.currentpoint();
            cy += inc / 2.0;
            if cy - y > inc / 2.1 {
                cy -= inc;
            }
            if cx > w || c.over() || advance <= 0.0 {
                break;
            }
            c.moveto(cx, cy);
        }
    }
    Some(())
}

/// A character's outline in one of PostScript's standard fonts (the closest installed font), one
/// unit high, y up, and its advance.
fn glyph(font: u8, ch: char) -> Option<(kurbo::BezPath, f64)> {
    use vector_core::text::{layout, CharFormat, TextObj};
    let (family, bold, italic) = match font {
        1..=4 => ("Times New Roman", font == 3 || font == 4, font == 2 || font == 4),
        5..=8 => ("Arial", font >= 7, font == 6 || font == 8),
        9..=12 => ("Courier New", font >= 11, font == 10 || font == 12),
        13 => ("Symbol", false, false),
        14..=17 => ("Century Gothic", font >= 16, font == 15 || font == 17),
        18..=21 => ("Bookman Old Style", font <= 19, font == 19 || font == 21),
        22..=25 => ("Arial Narrow", font == 23 || font == 24, font == 24 || font == 25),
        26..=29 => ("Century Schoolbook", font == 27 || font == 29, font >= 28),
        30..=33 => ("Palatino Linotype", font == 31 || font == 33, font >= 32),
        34 => ("Monotype Corsiva", false, true),
        _ => ("Wingdings", false, false),
    };
    let format = CharFormat { font: family.into(), size: 100.0, bold, italic, ..Default::default() };
    let t = TextObj::artistic(&ch.to_string(), format);
    let l = layout(&t);
    let g = l.glyphs.first()?;
    // points to one unit, baseline at 0, y up
    let flip = Affine::new([0.01, 0.0, 0.0, -0.01, -0.01 * g.origin.x, 0.01 * g.origin.y]);
    Some((flip * (g.transform * (*g.path).clone()), g.advance / 100.0))
}

/// Tiles: rows of tiles with rounded ends, half a tile up from one to the next.
fn tiles(c: &mut Canvas, lw: f64) {
    c.moveto(0.0, 0.0);
    c.lineto(1.5, 0.0);
    c.arcn(1.75, 0.0, 0.25, 180.0, 90.0);
    c.arc(1.75, 0.5, 0.25, -90.0, 90.0);
    c.arcn(1.75, 1.0, 0.25, 270.0, 180.0);
    c.lineto(0.0, 1.0);
    c.setlinewidth(lw);
    c.stroke();
}
