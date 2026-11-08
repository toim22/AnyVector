//! Bitmap tracing (Quick Trace / PowerTRACE): color quantization, speckle removal, stacked
//! boundary extraction and curve fitting. Produces filled curve objects over the bitmap.

use kurbo::{Affine, Point};
use std::collections::HashMap;
use vector_core::path::{fit_freehand, NodePath};
use vector_core::{Color, Fill, ObjKind, Object, Outline, Style};

#[derive(Clone, Debug)]
pub struct TraceParams {
    /// Number of colors (2 = black and white line art).
    pub colors: usize,
    /// 0..100: higher = smoother curves, fewer nodes.
    pub smoothing: f64,
    /// Regions smaller than this many pixels are merged into their neighbors.
    pub min_area: usize,
    /// Remove the background color (the dominant color along the border).
    pub remove_background: bool,
    /// Max side of the working bitmap (larger images are downsampled).
    pub max_side: u32,
}

impl Default for TraceParams {
    fn default() -> Self {
        TraceParams { colors: 8, smoothing: 50.0, min_area: 12, remove_background: true, max_side: 900 }
    }
}

/// Presets matching the Quick Trace methods.
pub fn preset(method: &str) -> TraceParams {
    let mut p = TraceParams::default();
    match method {
        "Line Art" => {
            p.colors = 2;
            p.smoothing = 35.0;
        }
        "Logo" => {
            p.colors = 6;
            p.smoothing = 45.0;
        }
        "Detailed Logo" => {
            p.colors = 12;
            p.smoothing = 30.0;
            p.min_area = 6;
        }
        "Low Quality Image" => {
            p.colors = 16;
            p.smoothing = 60.0;
            p.min_area = 30;
        }
        "High Quality Image" => {
            p.colors = 32;
            p.smoothing = 25.0;
            p.min_area = 4;
            p.max_side = 1400;
        }
        "Centerline" => {
            p.colors = 2;
            p.smoothing = 20.0;
            p.min_area = 4;
        }
        _ => {}
    }
    p
}

pub struct TraceResult {
    /// Objects in drawing order (bottom first), in document coordinates.
    pub objects: Vec<Object>,
    pub palette: Vec<Color>,
    pub node_count: usize,
}

/// Trace RGBA pixels. `to_doc` maps pixel coordinates to document coordinates.
pub fn trace(width: u32, height: u32, rgba: &[u8], params: &TraceParams, to_doc: Affine, mut next_id: impl FnMut() -> u64) -> TraceResult {
    // downsample
    let scale = (params.max_side as f64 / width.max(height) as f64).min(1.0);
    let w = ((width as f64 * scale).round() as usize).max(1);
    let h = ((height as f64 * scale).round() as usize).max(1);
    let mut px: Vec<[u8; 4]> = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f64 + 0.5) / scale).min(width as f64 - 1.0) as usize;
            let sy = ((y as f64 + 0.5) / scale).min(height as f64 - 1.0) as usize;
            let i = (sy * width as usize + sx) * 4;
            px.push([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
        }
    }
    let pixel_to_doc = to_doc * Affine::scale(1.0 / scale);

    // quantize (k-means on samples)
    let k = params.colors.clamp(2, 64);
    let centers = kmeans(&px, k);
    const NONE: u8 = 255;
    let mut labels: Vec<u8> = px.iter().map(|p| if p[3] < 128 { NONE } else { nearest(&centers, p) as u8 }).collect();

    // mode filter to remove isolated pixels
    labels = mode_filter(&labels, w, h);
    // merge small regions
    merge_small(&mut labels, w, h, params.min_area.max(1), NONE);

    // background removal: dominant label on the border
    if params.remove_background {
        let mut counts: HashMap<u8, usize> = HashMap::new();
        for x in 0..w {
            *counts.entry(labels[x]).or_default() += 1;
            *counts.entry(labels[(h - 1) * w + x]).or_default() += 1;
        }
        for y in 0..h {
            *counts.entry(labels[y * w]).or_default() += 1;
            *counts.entry(labels[y * w + w - 1]).or_default() += 1;
        }
        let total: usize = counts.values().sum();
        if let Some((&bg, &n)) = counts.iter().max_by_key(|(_, n)| **n) {
            if bg != NONE && n * 2 > total {
                for l in labels.iter_mut() {
                    if *l == bg {
                        *l = NONE;
                    }
                }
            }
        }
    }

    // order colors by area, largest first (bottom of the stack)
    let mut area = vec![0usize; centers.len()];
    for l in &labels {
        if *l != NONE {
            area[*l as usize] += 1;
        }
    }
    let mut order: Vec<usize> = (0..centers.len()).filter(|i| area[*i] > 0).collect();
    order.sort_by(|a, b| area[*b].cmp(&area[*a]));
    let mut rank = vec![usize::MAX; centers.len()];
    for (r, c) in order.iter().enumerate() {
        rank[*c] = r;
    }

    let mut objects = vec![];
    let mut palette = vec![];
    let mut node_count = 0;
    for (r, c) in order.iter().enumerate() {
        let above = |l: u8| l != NONE && rank[l as usize] > r;
        let mask = layer_mask(&labels, w, h, *c as u8, &above);
        let loops = boundaries(&mask, w, h);
        let mut np = NodePath::new();
        for lp in loops {
            if lp.len() < 4 {
                continue;
            }
            let pts = simplify_loop(&lp, 0.75);
            if pts.len() < 3 {
                continue;
            }
            let docpts: Vec<Point> = pts.iter().map(|p| pixel_to_doc * *p).collect();
            let fitted = if params.smoothing <= 0.5 { vector_core::path::polyline(&docpts, true) } else { fit_freehand(&docpts, params.smoothing * 0.35, true) };
            np.subpaths.extend(fitted.subpaths);
        }
        if np.is_empty() {
            continue;
        }
        node_count += np.node_count();
        let col = centers[*c];
        let color = Color::rgb(col[0], col[1], col[2]);
        palette.push(color);
        let mut style = Style::filled(color);
        style.outline = Outline::none();
        style.fill = Fill::Solid(color);
        style.even_odd = true;
        let mut o = Object::new(next_id(), ObjKind::Path(np), Affine::IDENTITY, style);
        o.name = format!("Traced {}", color.to_hex());
        objects.push(o);
    }
    TraceResult { objects, palette, node_count }
}

/// The pixels of one color, plus holes filled entirely by colors drawn above it, plus a one
/// pixel overlap under neighboring colors drawn above (so no hairline gaps remain).
fn layer_mask(labels: &[u8], w: usize, h: usize, color: u8, above: &dyn Fn(u8) -> bool) -> Vec<bool> {
    let mut mask: Vec<bool> = labels.iter().map(|l| *l == color).collect();
    // holes: components of !mask not touching the border
    let mut seen = vec![false; w * h];
    let mut stack = vec![];
    let mut comp = vec![];
    for start in 0..w * h {
        if mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        comp.clear();
        let mut border = false;
        let mut fillable = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                border = true;
            }
            if !above(labels[i]) {
                fillable = false;
            }
            let mut push = |j: usize| {
                if !mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        if !border && fillable {
            for &i in &comp {
                mask[i] = true;
            }
        }
    }
    // one pixel overlap under colors drawn above
    let base = mask.clone();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if base[i] || !above(labels[i]) {
                continue;
            }
            let near = (x > 0 && base[i - 1]) || (x + 1 < w && base[i + 1]) || (y > 0 && base[i - w]) || (y + 1 < h && base[i + w]);
            if near {
                mask[i] = true;
            }
        }
    }
    mask
}

fn dist2(a: &[u8; 3], b: &[u8; 4]) -> u32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    // perceptual-ish weights
    (2 * dr * dr + 4 * dg * dg + 3 * db * db) as u32
}

fn nearest(centers: &[[u8; 3]], p: &[u8; 4]) -> usize {
    let mut best = 0;
    let mut bd = u32::MAX;
    for (i, c) in centers.iter().enumerate() {
        let d = dist2(c, p);
        if d < bd {
            bd = d;
            best = i;
        }
    }
    best
}

fn kmeans(px: &[[u8; 4]], k: usize) -> Vec<[u8; 3]> {
    let step = (px.len() / 20000).max(1);
    let samples: Vec<[u8; 4]> = px.iter().step_by(step).filter(|p| p[3] >= 128).copied().collect();
    if samples.is_empty() {
        return vec![[0, 0, 0]];
    }
    // farthest-point initialization (deterministic)
    let mut centers: Vec<[u8; 3]> = vec![[samples[0][0], samples[0][1], samples[0][2]]];
    let mut dmin: Vec<u32> = samples.iter().map(|s| dist2(&centers[0], s)).collect();
    while centers.len() < k {
        let (i, d) = dmin.iter().enumerate().max_by_key(|(_, d)| **d).map(|(i, d)| (i, *d)).unwrap();
        if d == 0 {
            break;
        }
        let c = [samples[i][0], samples[i][1], samples[i][2]];
        centers.push(c);
        for (j, s) in samples.iter().enumerate() {
            dmin[j] = dmin[j].min(dist2(&c, s));
        }
    }
    for _ in 0..12 {
        let mut sum = vec![[0u64; 4]; centers.len()];
        for s in &samples {
            let i = nearest(&centers, s);
            sum[i][0] += s[0] as u64;
            sum[i][1] += s[1] as u64;
            sum[i][2] += s[2] as u64;
            sum[i][3] += 1;
        }
        let mut moved = false;
        for (i, c) in centers.iter_mut().enumerate() {
            if sum[i][3] > 0 {
                let n = [(sum[i][0] / sum[i][3]) as u8, (sum[i][1] / sum[i][3]) as u8, (sum[i][2] / sum[i][3]) as u8];
                if n != *c {
                    moved = true;
                    *c = n;
                }
            }
        }
        if !moved {
            break;
        }
    }
    centers
}

fn mode_filter(labels: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = labels.to_vec();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let c = labels[y * w + x];
            let mut same = 0;
            let mut counts: [(u8, u8); 8] = [(0, 0); 8];
            let mut n = 0;
            for dy in [-1i32, 0, 1] {
                for dx in [-1i32, 0, 1] {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let l = labels[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize];
                    if l == c {
                        same += 1;
                    } else if let Some(e) = counts[..n].iter_mut().find(|e| e.0 == l) {
                        e.1 += 1;
                    } else {
                        counts[n] = (l, 1);
                        n += 1;
                    }
                }
            }
            if same <= 1 {
                if let Some(best) = counts[..n].iter().max_by_key(|e| e.1) {
                    if best.1 >= 5 {
                        out[y * w + x] = best.0;
                    }
                }
            }
        }
    }
    out
}

fn merge_small(labels: &mut [u8], w: usize, h: usize, min_area: usize, none: u8) {
    if min_area <= 1 {
        return;
    }
    let mut comp = vec![u32::MAX; w * h];
    let mut stack = vec![];
    let mut members = vec![];
    for start in 0..w * h {
        if comp[start] != u32::MAX {
            continue;
        }
        let l = labels[start];
        comp[start] = start as u32;
        stack.clear();
        members.clear();
        stack.push(start);
        let mut neighbors: HashMap<u8, usize> = HashMap::new();
        while let Some(i) = stack.pop() {
            members.push(i);
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize, stack: &mut Vec<usize>, comp: &mut [u32]| {
                if labels[j] == l {
                    if comp[j] == u32::MAX {
                        comp[j] = start as u32;
                        stack.push(j);
                    }
                } else {
                    *neighbors.entry(labels[j]).or_default() += 1;
                }
            };
            if x > 0 {
                visit(i - 1, &mut stack, &mut comp);
            }
            if x + 1 < w {
                visit(i + 1, &mut stack, &mut comp);
            }
            if y > 0 {
                visit(i - w, &mut stack, &mut comp);
            }
            if y + 1 < h {
                visit(i + w, &mut stack, &mut comp);
            }
        }
        if members.len() < min_area && l != none {
            if let Some((&nl, _)) = neighbors.iter().filter(|(k, _)| **k != none).max_by_key(|(_, n)| **n) {
                for &m in &members {
                    labels[m] = nl;
                }
            }
        }
    }
}

/// Extract closed boundary loops of a binary mask by following pixel cracks
/// (outer boundaries clockwise, holes counter-clockwise in y-down coordinates).
fn boundaries(mask: &[bool], w: usize, h: usize) -> Vec<Vec<Point>> {
    let inside = |x: i64, y: i64| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && mask[y as usize * w + x as usize];
    // directed edges keyed by start vertex; direction: 0=+x,1=+y,2=-x,3=-y
    let vw = w + 1;
    let mut out: HashMap<usize, Vec<u8>> = HashMap::new();
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !inside(x, y) {
                continue;
            }
            let v = |x: i64, y: i64| y as usize * vw + x as usize;
            if !inside(x, y - 1) {
                out.entry(v(x, y)).or_default().push(0);
            }
            if !inside(x + 1, y) {
                out.entry(v(x + 1, y)).or_default().push(1);
            }
            if !inside(x, y + 1) {
                out.entry(v(x + 1, y + 1)).or_default().push(2);
            }
            if !inside(x - 1, y) {
                out.entry(v(x, y + 1)).or_default().push(3);
            }
        }
    }
    let mut loops = vec![];
    let mut starts: Vec<usize> = out.keys().copied().collect();
    starts.sort_unstable();
    for s in starts {
        loop {
            let Some(dirs) = out.get_mut(&s) else { break };
            let Some(d0) = dirs.pop() else { break };
            let mut pts = vec![];
            let mut v = s;
            let mut d = d0;
            let mut guard = 0;
            loop {
                let (x, y) = ((v % vw) as f64, (v / vw) as f64);
                pts.push(Point::new(x, y));
                let nv = match d {
                    0 => v + 1,
                    1 => v + vw,
                    2 => v - 1,
                    _ => v - vw,
                };
                v = nv;
                if v == s {
                    break;
                }
                guard += 1;
                if guard > 4 * (w + 1) * (h + 1) {
                    break;
                }
                let Some(dirs) = out.get_mut(&v) else { break };
                // at saddles prefer turning right (keeps diagonal pixels separate)
                let prefs = [(d + 1) % 4, d, (d + 3) % 4];
                let mut chosen = None;
                for p in prefs {
                    if let Some(i) = dirs.iter().position(|x| *x == p) {
                        chosen = Some(dirs.swap_remove(i));
                        break;
                    }
                }
                match chosen {
                    Some(nd) => d = nd,
                    None => break,
                }
            }
            if pts.len() >= 4 {
                loops.push(pts);
            }
        }
    }
    loops
}

/// Midpoint smoothing of the staircase followed by Ramer–Douglas–Peucker.
fn simplify_loop(pts: &[Point], tol: f64) -> Vec<Point> {
    let n = pts.len();
    let mids: Vec<Point> = (0..n).map(|i| pts[i].midpoint(pts[(i + 1) % n])).collect();
    // RDP on a closed loop: split at the point farthest from the first
    let far = (0..mids.len()).max_by(|a, b| (mids[*a] - mids[0]).hypot2().partial_cmp(&(mids[*b] - mids[0]).hypot2()).unwrap()).unwrap_or(0);
    let mut a = rdp(&mids[..=far], tol);
    let mut tail: Vec<Point> = mids[far..].to_vec();
    tail.push(mids[0]);
    let b = rdp(&tail, tol);
    a.pop();
    a.extend(b);
    a.pop();
    a
}

fn rdp(pts: &[Point], tol: f64) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let mut idx = 0;
    let mut dmax = 0.0;
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = vector_core::geom::dist_point_segment(*p, a, b);
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax > tol {
        let mut l = rdp(&pts[..=idx], tol);
        let r = rdp(&pts[idx..], tol);
        l.pop();
        l.extend(r);
        l
    } else {
        vec![a, b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_two_squares() {
        let (w, h) = (40u32, 20u32);
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        for y in 5..15 {
            for x in 5..15 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&[255, 0, 0]);
            }
            for x in 25..35 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&[0, 0, 255]);
            }
        }
        let mut id = 0;
        let p = TraceParams { colors: 3, smoothing: 0.0, min_area: 2, remove_background: true, max_side: 100 };
        let r = trace(w, h, &rgba, &p, Affine::IDENTITY, || {
            id += 1;
            id
        });
        assert_eq!(r.objects.len(), 2, "red and blue, white removed");
        for o in &r.objects {
            let b = o.bounds();
            assert!(b.width() > 8.0 && b.width() < 11.0, "{b:?}");
        }
    }
}
