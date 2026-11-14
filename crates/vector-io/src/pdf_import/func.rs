//! PDF functions (types 0, 2, 3 and 4), used by shadings and by the tint transforms of
//! Separation and DeviceN color spaces.

use super::Pdf;
use lopdf::Object;

#[derive(Debug)]
pub enum Function {
    Sampled { domain: Vec<f64>, range: Vec<f64>, size: Vec<usize>, encode: Vec<f64>, decode: Vec<f64>, samples: Vec<f64> },
    Exponential { domain: Vec<f64>, c0: Vec<f64>, c1: Vec<f64>, n: f64 },
    Stitching { domain: Vec<f64>, funcs: Vec<Function>, bounds: Vec<f64>, encode: Vec<f64> },
    PostScript { domain: Vec<f64>, range: Vec<f64>, prog: Vec<Ps> },
    /// An array of one-output functions, one per color component.
    Array(Vec<Function>),
}

fn clip(x: f64, a: f64, b: f64) -> f64 {
    if a <= b {
        x.clamp(a, b)
    } else {
        x.clamp(b, a)
    }
}

fn interp(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    if (x1 - x0).abs() < 1e-12 {
        y0
    } else {
        y0 + (x - x0) * (y1 - y0) / (x1 - x0)
    }
}

impl Function {
    pub fn parse(pdf: &Pdf, o: &Object) -> Option<Function> {
        Self::parse_depth(pdf, o, 0)
    }

    fn parse_depth(pdf: &Pdf, o: &Object, depth: usize) -> Option<Function> {
        if depth > 8 {
            return None;
        }
        let o = pdf.deref(o);
        if let Object::Array(a) = o {
            let fs: Vec<Function> = a.iter().filter_map(|f| Self::parse_depth(pdf, f, depth + 1)).collect();
            return (!fs.is_empty()).then_some(Function::Array(fs));
        }
        let d = pdf.dict(o)?;
        let ty = pdf.int(d, b"FunctionType")?;
        let domain = pdf.nums(d, b"Domain");
        let domain = if domain.len() >= 2 { domain } else { vec![0.0, 1.0] };
        match ty {
            0 => {
                let size: Vec<usize> = pdf.nums(d, b"Size").iter().map(|v| v.max(1.0) as usize).collect();
                let range = pdf.nums(d, b"Range");
                let bps = pdf.int(d, b"BitsPerSample").unwrap_or(8).clamp(1, 32) as u32;
                let m = size.len();
                let n = range.len() / 2;
                if m == 0 || n == 0 {
                    return None;
                }
                let mut encode = pdf.nums(d, b"Encode");
                if encode.len() < 2 * m {
                    encode = size.iter().flat_map(|s| [0.0, (*s as f64 - 1.0)]).collect();
                }
                let mut decode = pdf.nums(d, b"Decode");
                if decode.len() < 2 * n {
                    decode = range.clone();
                }
                let data = pdf.stream_data(o)?;
                let count: usize = size.iter().product::<usize>() * n;
                let max = ((1u64 << bps) - 1) as f64;
                let mut samples = Vec::with_capacity(count.min(1 << 20));
                let mut bits = BitReader { d: &data, pos: 0 };
                for _ in 0..count.min(1 << 22) {
                    let Some(v) = bits.read(bps) else { break };
                    samples.push(v as f64 / max);
                }
                Some(Function::Sampled { domain, range, size, encode, decode, samples })
            }
            2 => {
                let c0 = pdf.nums(d, b"C0");
                let c1 = pdf.nums(d, b"C1");
                let c0 = if c0.is_empty() { vec![0.0] } else { c0 };
                let c1 = if c1.is_empty() { vec![1.0] } else { c1 };
                let n = pdf.num(d, b"N").unwrap_or(1.0);
                Some(Function::Exponential { domain, c0, c1, n })
            }
            3 => {
                let funcs: Vec<Function> = pdf.array(d, b"Functions").iter().filter_map(|f| Self::parse_depth(pdf, f, depth + 1)).collect();
                if funcs.is_empty() {
                    return None;
                }
                let bounds = pdf.nums(d, b"Bounds");
                let mut encode = pdf.nums(d, b"Encode");
                if encode.len() < 2 * funcs.len() {
                    encode = funcs.iter().flat_map(|_| [0.0, 1.0]).collect();
                }
                Some(Function::Stitching { domain, funcs, bounds, encode })
            }
            4 => {
                let data = pdf.stream_data(o)?;
                let prog = parse_ps(&data)?;
                Some(Function::PostScript { domain, range: pdf.nums(d, b"Range"), prog })
            }
            _ => None,
        }
    }

    /// The function's outputs for the inputs.
    pub fn eval(&self, input: &[f64]) -> Vec<f64> {
        match self {
            Function::Array(fs) => fs.iter().flat_map(|f| f.eval(input).into_iter().take(1)).collect(),
            Function::Exponential { domain, c0, c1, n } => {
                let x = clip(input.first().copied().unwrap_or(0.0), domain[0], domain[1]);
                let k = c0.len().min(c1.len());
                (0..k).map(|i| c0[i] + x.powf(*n) * (c1[i] - c0[i])).collect()
            }
            Function::Stitching { domain, funcs, bounds, encode } => {
                let x = clip(input.first().copied().unwrap_or(0.0), domain[0], domain[1]);
                let k = funcs.len();
                let mut i = 0;
                while i < bounds.len() && i + 1 < k && x >= bounds[i] {
                    i += 1;
                }
                let lo = if i == 0 { domain[0] } else { bounds[i - 1] };
                let hi = if i < bounds.len() { bounds[i] } else { domain[1] };
                let e = interp(x, lo, hi, encode[2 * i], encode[2 * i + 1]);
                funcs[i].eval(&[e])
            }
            Function::Sampled { domain, range, size, encode, decode, samples } => {
                let m = size.len();
                let n = range.len() / 2;
                // sample position along each input (linear interpolation along the first one)
                let mut idx = vec![0f64; m];
                for j in 0..m {
                    let x = clip(input.get(j).copied().unwrap_or(0.0), domain[2 * j], domain.get(2 * j + 1).copied().unwrap_or(1.0));
                    let e = interp(x, domain[2 * j], domain.get(2 * j + 1).copied().unwrap_or(1.0), encode[2 * j], encode[2 * j + 1]);
                    idx[j] = e.clamp(0.0, size[j] as f64 - 1.0);
                }
                let offset = |pos: &[usize]| -> usize {
                    let mut off = 0;
                    let mut stride = 1;
                    for j in 0..m {
                        off += pos[j] * stride;
                        stride *= size[j];
                    }
                    off * n
                };
                let base: Vec<usize> = idx.iter().map(|v| v.floor() as usize).collect();
                let frac = idx[0] - base[0] as f64;
                let mut next = base.clone();
                next[0] = (base[0] + 1).min(size[0] - 1);
                let (o0, o1) = (offset(&base), offset(&next));
                (0..n)
                    .map(|i| {
                        let a = samples.get(o0 + i).copied().unwrap_or(0.0);
                        let b = samples.get(o1 + i).copied().unwrap_or(a);
                        let s = a + (b - a) * frac;
                        let v = decode[2 * i] + s * (decode[2 * i + 1] - decode[2 * i]);
                        clip(v, range[2 * i], range[2 * i + 1])
                    })
                    .collect()
            }
            Function::PostScript { domain, range, prog } => {
                let mut st: Vec<f64> = input.iter().enumerate().map(|(i, x)| clip(*x, domain.get(2 * i).copied().unwrap_or(0.0), domain.get(2 * i + 1).copied().unwrap_or(1.0))).collect();
                run_ps(prog, &mut st, 0);
                let n = range.len() / 2;
                let start = st.len().saturating_sub(n);
                st[start..].iter().enumerate().map(|(i, v)| clip(*v, range[2 * i], range[2 * i + 1])).collect()
            }
        }
    }

    /// The input values where the output changes slope sharply (stitching bounds), so a
    /// sampled gradient keeps its hard steps.
    pub fn breaks(&self) -> Vec<f64> {
        match self {
            Function::Stitching { bounds, .. } => bounds.clone(),
            Function::Array(fs) => fs.iter().flat_map(|f| f.breaks()).collect(),
            _ => vec![],
        }
    }
}

struct BitReader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn read(&mut self, bits: u32) -> Option<u32> {
        let mut v: u64 = 0;
        for _ in 0..bits {
            let byte = *self.d.get(self.pos / 8)?;
            let bit = (byte >> (7 - (self.pos % 8))) & 1;
            v = (v << 1) | bit as u64;
            self.pos += 1;
        }
        Some(v as u32)
    }
}

// ------------------------------------------------------------------------------------------
// Type 4: PostScript calculator
// ------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Ps {
    Num(f64),
    Op(String),
    If(Vec<Ps>),
    IfElse(Vec<Ps>, Vec<Ps>),
}

fn parse_ps(data: &[u8]) -> Option<Vec<Ps>> {
    let text = String::from_utf8_lossy(data).replace('{', " { ").replace('}', " } ");
    let toks: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    // the program is one {...} block
    while i < toks.len() && toks[i] != "{" {
        i += 1;
    }
    i += 1;
    let (prog, _) = parse_block(&toks, i, 0)?;
    Some(prog)
}

fn parse_block(toks: &[&str], mut i: usize, depth: usize) -> Option<(Vec<Ps>, usize)> {
    if depth > 32 {
        return None;
    }
    let mut out = vec![];
    let mut pending: Vec<Vec<Ps>> = vec![];
    while i < toks.len() {
        let t = toks[i];
        i += 1;
        match t {
            "{" => {
                let (b, ni) = parse_block(toks, i, depth + 1)?;
                pending.push(b);
                i = ni;
            }
            "}" => return Some((out, i)),
            "if" => {
                let b = pending.pop()?;
                out.push(Ps::If(b));
            }
            "ifelse" => {
                let b2 = pending.pop()?;
                let b1 = pending.pop()?;
                out.push(Ps::IfElse(b1, b2));
            }
            "true" => out.push(Ps::Num(1.0)),
            "false" => out.push(Ps::Num(0.0)),
            _ => match t.parse::<f64>() {
                Ok(v) => out.push(Ps::Num(v)),
                Err(_) => out.push(Ps::Op(t.to_string())),
            },
        }
    }
    Some((out, i))
}

fn run_ps(prog: &[Ps], st: &mut Vec<f64>, depth: usize) {
    if depth > 32 {
        return;
    }
    for p in prog {
        if st.len() > 1000 {
            return;
        }
        match p {
            Ps::Num(v) => st.push(*v),
            Ps::If(b) => {
                if st.pop().unwrap_or(0.0) != 0.0 {
                    run_ps(b, st, depth + 1);
                }
            }
            Ps::IfElse(a, b) => {
                if st.pop().unwrap_or(0.0) != 0.0 {
                    run_ps(a, st, depth + 1);
                } else {
                    run_ps(b, st, depth + 1);
                }
            }
            Ps::Op(op) => ps_op(op, st),
        }
    }
}

fn ps_op(op: &str, st: &mut Vec<f64>) {
    let pop = |st: &mut Vec<f64>| st.pop().unwrap_or(0.0);
    let b2f = |b: bool| if b { 1.0 } else { 0.0 };
    match op {
        "add" | "sub" | "mul" | "div" | "idiv" | "mod" | "exp" | "atan" | "eq" | "ne" | "gt" | "ge" | "lt" | "le" | "and" | "or" | "xor" | "bitshift" => {
            let b = pop(st);
            let a = pop(st);
            st.push(match op {
                "add" => a + b,
                "sub" => a - b,
                "mul" => a * b,
                "div" => {
                    if b == 0.0 {
                        0.0
                    } else {
                        a / b
                    }
                }
                "idiv" => {
                    if b as i64 == 0 {
                        0.0
                    } else {
                        ((a as i64) / (b as i64)) as f64
                    }
                }
                "mod" => {
                    if b as i64 == 0 {
                        0.0
                    } else {
                        ((a as i64) % (b as i64)) as f64
                    }
                }
                "exp" => a.powf(b),
                "atan" => {
                    let d = a.atan2(b).to_degrees();
                    if d < 0.0 {
                        d + 360.0
                    } else {
                        d
                    }
                }
                "eq" => b2f(a == b),
                "ne" => b2f(a != b),
                "gt" => b2f(a > b),
                "ge" => b2f(a >= b),
                "lt" => b2f(a < b),
                "le" => b2f(a <= b),
                "and" => ((a as i64) & (b as i64)) as f64,
                "or" => ((a as i64) | (b as i64)) as f64,
                "xor" => ((a as i64) ^ (b as i64)) as f64,
                _ => {
                    let (a, b) = (a as i64, b as i64);
                    (if b >= 0 { a << b.min(63) } else { a >> (-b).min(63) }) as f64
                }
            });
        }
        "neg" | "abs" | "ceiling" | "floor" | "round" | "truncate" | "sqrt" | "sin" | "cos" | "ln" | "log" | "cvi" | "cvr" | "not" => {
            let a = pop(st);
            st.push(match op {
                "neg" => -a,
                "abs" => a.abs(),
                "ceiling" => a.ceil(),
                "floor" => a.floor(),
                "round" => (a + 0.5).floor(),
                "truncate" | "cvi" => a.trunc(),
                "sqrt" => a.max(0.0).sqrt(),
                "sin" => a.to_radians().sin(),
                "cos" => a.to_radians().cos(),
                "ln" => a.ln(),
                "log" => a.log10(),
                "not" => {
                    if a == 0.0 {
                        1.0
                    } else if a == 1.0 {
                        0.0
                    } else {
                        !(a as i64) as f64
                    }
                }
                _ => a,
            });
        }
        "dup" => {
            let a = st.last().copied().unwrap_or(0.0);
            st.push(a);
        }
        "pop" => {
            st.pop();
        }
        "exch" => {
            let n = st.len();
            if n >= 2 {
                st.swap(n - 1, n - 2);
            }
        }
        "copy" => {
            let k = pop(st).max(0.0) as usize;
            let n = st.len();
            if k <= n {
                let v: Vec<f64> = st[n - k..].to_vec();
                st.extend(v);
            }
        }
        "index" => {
            let k = pop(st).max(0.0) as usize;
            let n = st.len();
            let v = if k < n { st[n - 1 - k] } else { 0.0 };
            st.push(v);
        }
        "roll" => {
            let j = pop(st) as i64;
            let k = pop(st).max(0.0) as usize;
            let n = st.len();
            if k > 0 && k <= n {
                let s = &mut st[n - k..];
                let r = j.rem_euclid(k as i64) as usize;
                s.rotate_right(r);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postscript_calculator() {
        let prog = parse_ps(b"{ dup 0.5 mul exch 1 exch sub 2 copy gt { pop } { exch pop } ifelse }").unwrap();
        let f = Function::PostScript { domain: vec![0.0, 1.0], range: vec![0.0, 1.0, 0.0, 1.0], prog };
        // x=0.4: 0.2, 0.6 -> 2 copy gt false -> exch pop -> 0.2 0.6 -> exch pop -> 0.6? (stack: 0.2 0.6, copy -> 0.2 0.6 0.2 0.6, gt -> 0.2 0.6 0, else: exch pop -> 0.6)
        let v = f.eval(&[0.4]);
        assert_eq!(v.len(), 1);
        assert!((v[0] - 0.6).abs() < 1e-9);
    }

    #[test]
    fn exponential_and_stitching() {
        let a = Function::Exponential { domain: vec![0.0, 1.0], c0: vec![0.0, 0.0, 0.0], c1: vec![1.0, 0.5, 0.0], n: 1.0 };
        assert_eq!(a.eval(&[0.5]), vec![0.5, 0.25, 0.0]);
        let s = Function::Stitching {
            domain: vec![0.0, 1.0],
            funcs: vec![
                Function::Exponential { domain: vec![0.0, 1.0], c0: vec![0.0], c1: vec![1.0], n: 1.0 },
                Function::Exponential { domain: vec![0.0, 1.0], c0: vec![1.0], c1: vec![0.0], n: 1.0 },
            ],
            bounds: vec![0.5],
            encode: vec![0.0, 1.0, 0.0, 1.0],
        };
        assert!((s.eval(&[0.25])[0] - 0.5).abs() < 1e-9);
        assert!((s.eval(&[0.75])[0] - 0.5).abs() < 1e-9);
        assert!((s.eval(&[0.5])[0] - 1.0).abs() < 1e-9);
    }
}
