//! The operators that are not about graphics: stacks, arithmetic, control, types, dictionaries,
//! arrays, strings, files, VM, resources and the device settings EPS prologs ask about.

use super::interp::{op, Flow, Interp, OpFn, PsErr, R};
use super::obj::*;
use std::rc::Rc;

pub fn ops() -> Vec<(&'static str, OpFn)> {
    vec![
        // ------------------------------------------------ operand stack
        op("pop", |i| i.pop().map(|_| ())),
        op("exch", |i| {
            i.need(2)?;
            let n = i.os.len();
            i.os.swap(n - 1, n - 2);
            Ok(())
        }),
        op("dup", |i| {
            let o = i.os.last().cloned().ok_or(Flow::Err(PsErr::StackUnderflow))?;
            i.push(o);
            Ok(())
        }),
        op("copy", copy),
        op("index", |i| {
            let n = i.pop_usize()?;
            if n >= i.os.len() {
                return Err(PsErr::RangeCheck.into());
            }
            let o = i.os[i.os.len() - 1 - n].clone();
            i.push(o);
            Ok(())
        }),
        op("roll", |i| {
            let j = i.pop_int()?;
            let n = i.pop_usize()?;
            if n > i.os.len() {
                return Err(PsErr::RangeCheck.into());
            }
            if n > 0 {
                let at = i.os.len() - n;
                let s = &mut i.os[at..];
                let k = j.rem_euclid(n as i64) as usize;
                s.rotate_right(k);
            }
            Ok(())
        }),
        op("clear", |i| {
            i.os.clear();
            Ok(())
        }),
        op("count", |i| {
            i.push(Obj::Int(i.os.len() as i64));
            Ok(())
        }),
        op("mark", |i| {
            i.push(Obj::Mark);
            Ok(())
        }),
        op("[", |i| {
            i.push(Obj::Mark);
            Ok(())
        }),
        op("<<", |i| {
            i.push(Obj::Mark);
            Ok(())
        }),
        op("cleartomark", |i| {
            let m = mark_pos(i)?;
            i.os.truncate(m);
            Ok(())
        }),
        op("counttomark", |i| {
            let m = mark_pos(i)?;
            i.push(Obj::Int((i.os.len() - m - 1) as i64));
            Ok(())
        }),
        op("]", |i| {
            let m = mark_pos(i)?;
            let items: Vec<Obj> = i.os.drain(m + 1..).collect();
            i.os.pop();
            i.push(Obj::arr(items));
            Ok(())
        }),
        op(">>", |i| {
            let m = mark_pos(i)?;
            let items: Vec<Obj> = i.os.drain(m + 1..).collect();
            i.os.pop();
            if items.len() % 2 != 0 {
                return Err(PsErr::RangeCheck.into());
            }
            let d = new_dict(items.len() / 2);
            for kv in items.chunks(2) {
                let k = i.to_key(&kv[0])?;
                d.borrow_mut().map.insert(k, kv[1].clone());
            }
            i.push(Obj::Dict(d));
            Ok(())
        }),
        // ------------------------------------------------ arithmetic
        op("add", |i| arith(i, |a, b| a.checked_add(b), |a, b| a + b)),
        op("sub", |i| arith(i, |a, b| a.checked_sub(b), |a, b| a - b)),
        op("mul", |i| arith(i, |a, b| a.checked_mul(b), |a, b| a * b)),
        op("div", |i| {
            let b = i.pop_num()?;
            let a = i.pop_num()?;
            if b == 0.0 {
                return Err(PsErr::UndefinedResult.into());
            }
            i.push(Obj::Real(a / b));
            Ok(())
        }),
        op("idiv", |i| {
            let b = i.pop_int()?;
            let a = i.pop_int()?;
            if b == 0 {
                return Err(PsErr::UndefinedResult.into());
            }
            i.push(Obj::Int(a.wrapping_div(b)));
            Ok(())
        }),
        op("mod", |i| {
            let b = i.pop_int()?;
            let a = i.pop_int()?;
            if b == 0 {
                return Err(PsErr::UndefinedResult.into());
            }
            i.push(Obj::Int(a.wrapping_rem(b)));
            Ok(())
        }),
        op("abs", |i| unary(i, |a| a.checked_abs(), f64::abs)),
        op("neg", |i| unary(i, |a| a.checked_neg(), |a| -a)),
        op("ceiling", |i| unary(i, Some, f64::ceil)),
        op("floor", |i| unary(i, Some, f64::floor)),
        op("round", |i| unary(i, Some, |a| (a + 0.5).floor())),
        op("truncate", |i| unary(i, Some, f64::trunc)),
        op("sqrt", |i| {
            let a = i.pop_num()?;
            if a < 0.0 {
                return Err(PsErr::RangeCheck.into());
            }
            i.push(Obj::Real(a.sqrt()));
            Ok(())
        }),
        op("atan", |i| {
            let den = i.pop_num()?;
            let num = i.pop_num()?;
            if num == 0.0 && den == 0.0 {
                return Err(PsErr::UndefinedResult.into());
            }
            i.push(Obj::Real(num.atan2(den).to_degrees().rem_euclid(360.0)));
            Ok(())
        }),
        op("cos", |i| {
            let a = i.pop_num()?;
            i.push(Obj::Real(a.to_radians().cos()));
            Ok(())
        }),
        op("sin", |i| {
            let a = i.pop_num()?;
            i.push(Obj::Real(a.to_radians().sin()));
            Ok(())
        }),
        op("exp", |i| {
            let e = i.pop_num()?;
            let b = i.pop_num()?;
            let v = b.powf(e);
            if !v.is_finite() {
                return Err(PsErr::UndefinedResult.into());
            }
            i.push(Obj::Real(v));
            Ok(())
        }),
        op("ln", |i| {
            let a = i.pop_num()?;
            if a <= 0.0 {
                return Err(PsErr::RangeCheck.into());
            }
            i.push(Obj::Real(a.ln()));
            Ok(())
        }),
        op("log", |i| {
            let a = i.pop_num()?;
            if a <= 0.0 {
                return Err(PsErr::RangeCheck.into());
            }
            i.push(Obj::Real(a.log10()));
            Ok(())
        }),
        op("rand", |i| {
            i.rand = i.rand.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fff_ffff;
            i.push(Obj::Int(i.rand as i64));
            Ok(())
        }),
        op("srand", |i| {
            i.rand = i.pop_int()? as u32;
            Ok(())
        }),
        op("rrand", |i| {
            i.push(Obj::Int(i.rand as i64));
            Ok(())
        }),
        op("cvi", |i| {
            let o = i.pop()?;
            let v = match o {
                Obj::Int(v) => v as f64,
                Obj::Real(v) => v,
                Obj::Str(s, _) => match super::interp::parse_number(s.text().trim().as_bytes()) {
                    Some(n) => n.num().unwrap_or(0.0),
                    None => return Err(PsErr::SyntaxError.into()),
                },
                _ => return Err(PsErr::TypeCheck.into()),
            };
            if !v.is_finite() || v.abs() > 2147483647.0 {
                return Err(PsErr::RangeCheck.into());
            }
            i.push(Obj::Int(v.trunc() as i64));
            Ok(())
        }),
        op("cvr", |i| {
            let o = i.pop()?;
            let v = match o {
                Obj::Int(v) => v as f64,
                Obj::Real(v) => v,
                Obj::Str(s, _) => match super::interp::parse_number(s.text().trim().as_bytes()) {
                    Some(n) => n.num().unwrap_or(0.0),
                    None => return Err(PsErr::SyntaxError.into()),
                },
                _ => return Err(PsErr::TypeCheck.into()),
            };
            i.push(Obj::Real(v));
            Ok(())
        }),
        // ------------------------------------------------ relational, boolean, bitwise
        op("eq", |i| {
            let b = i.pop()?;
            let a = i.pop()?;
            let r = i.eq(&a, &b);
            i.push(Obj::Bool(r));
            Ok(())
        }),
        op("ne", |i| {
            let b = i.pop()?;
            let a = i.pop()?;
            let r = !i.eq(&a, &b);
            i.push(Obj::Bool(r));
            Ok(())
        }),
        op("ge", |i| compare(i, |o| o != std::cmp::Ordering::Less)),
        op("gt", |i| compare(i, |o| o == std::cmp::Ordering::Greater)),
        op("le", |i| compare(i, |o| o != std::cmp::Ordering::Greater)),
        op("lt", |i| compare(i, |o| o == std::cmp::Ordering::Less)),
        op("and", |i| logic(i, |a, b| a & b, |a, b| a && b)),
        op("or", |i| logic(i, |a, b| a | b, |a, b| a || b)),
        op("xor", |i| logic(i, |a, b| a ^ b, |a, b| a ^ b)),
        op("not", |i| match i.pop()? {
            Obj::Bool(b) => {
                i.push(Obj::Bool(!b));
                Ok(())
            }
            Obj::Int(v) => {
                i.push(Obj::Int(!(v as i32) as i64));
                Ok(())
            }
            _ => Err(PsErr::TypeCheck.into()),
        }),
        op("bitshift", |i| {
            let s = i.pop_int()?;
            let v = i.pop_int()? as i32 as u32;
            let r = if s >= 32 || s <= -32 {
                0
            } else if s >= 0 {
                v << s
            } else {
                v >> (-s)
            };
            i.push(Obj::Int(r as i32 as i64));
            Ok(())
        }),
        op("true", |i| {
            i.push(Obj::Bool(true));
            Ok(())
        }),
        op("false", |i| {
            i.push(Obj::Bool(false));
            Ok(())
        }),
        op("null", |i| {
            i.push(Obj::Null);
            Ok(())
        }),
        // ------------------------------------------------ control
        op("exec", |i| {
            let o = i.pop()?;
            i.exec_value(o)
        }),
        op("if", |i| {
            let p = i.pop_proc()?;
            let c = i.pop_bool()?;
            if c {
                i.exec_value(p.with_exec(true))?;
            }
            Ok(())
        }),
        op("ifelse", |i| {
            let b = i.pop_proc()?;
            let a = i.pop_proc()?;
            let c = i.pop_bool()?;
            i.exec_value(if c { a } else { b }.with_exec(true))
        }),
        op("for", |i| {
            let p = i.pop_proc()?.with_exec(true);
            let lim = i.pop()?;
            let inc = i.pop()?;
            let ini = i.pop()?;
            match (&ini, &inc, &lim) {
                (Obj::Int(a), Obj::Int(b), _) if lim.num().is_some() => {
                    let (mut v, b, l) = (*a, *b, lim.num().unwrap());
                    if b == 0 {
                        return Ok(());
                    }
                    while (b > 0 && v as f64 <= l) || (b < 0 && v as f64 >= l) {
                        i.push(Obj::Int(v));
                        match i.exec_value(p.clone()) {
                            Err(Flow::Exit) => break,
                            r => r?,
                        }
                        v += b;
                    }
                }
                _ => {
                    let (Some(mut v), Some(b), Some(l)) = (ini.num(), inc.num(), lim.num()) else { return Err(PsErr::TypeCheck.into()) };
                    if b == 0.0 {
                        return Ok(());
                    }
                    let mut n = 0u64;
                    while (b > 0.0 && v <= l + 1e-9 * b.abs()) || (b < 0.0 && v >= l - 1e-9 * b.abs()) {
                        i.push(Obj::Real(v));
                        match i.exec_value(p.clone()) {
                            Err(Flow::Exit) => break,
                            r => r?,
                        }
                        n += 1;
                        v = ini.num().unwrap() + b * n as f64;
                    }
                }
            }
            Ok(())
        }),
        op("repeat", |i| {
            let p = i.pop_proc()?.with_exec(true);
            let n = i.pop_int()?;
            if n < 0 {
                return Err(PsErr::RangeCheck.into());
            }
            for _ in 0..n {
                match i.exec_value(p.clone()) {
                    Err(Flow::Exit) => break,
                    r => r?,
                }
            }
            Ok(())
        }),
        op("loop", |i| {
            let p = i.pop_proc()?.with_exec(true);
            loop {
                match i.exec_value(p.clone()) {
                    Err(Flow::Exit) => break,
                    r => r?,
                }
            }
            Ok(())
        }),
        op("forall", |i| {
            let p = i.pop_proc()?.with_exec(true);
            let o = i.pop()?;
            match o {
                Obj::Array(a, _) => {
                    for k in 0..a.len {
                        let Some(x) = a.get(k) else { break };
                        i.push(x);
                        match i.exec_value(p.clone()) {
                            Err(Flow::Exit) => break,
                            r => r?,
                        }
                    }
                }
                Obj::Str(s, _) => {
                    for b in s.bytes() {
                        i.push(Obj::Int(b as i64));
                        match i.exec_value(p.clone()) {
                            Err(Flow::Exit) => break,
                            r => r?,
                        }
                    }
                }
                Obj::Dict(d) => {
                    let pairs: Vec<(Key, Obj)> = d.borrow().map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    for (k, v) in pairs {
                        let ko = i.key_obj(&k);
                        i.push(ko);
                        i.push(v);
                        match i.exec_value(p.clone()) {
                            Err(Flow::Exit) => break,
                            r => r?,
                        }
                    }
                }
                _ => return Err(PsErr::TypeCheck.into()),
            }
            Ok(())
        }),
        op("exit", |_| Err(Flow::Exit)),
        op("stop", |_| Err(Flow::Stop)),
        op("stopped", |i| {
            let o = i.pop()?;
            i.stopped += 1;
            let base_ds = i.ds.len();
            let r = i.exec_value(o);
            i.stopped -= 1;
            match r {
                Ok(()) => i.push(Obj::Bool(false)),
                Err(Flow::Stop) => {
                    // a stopped procedure may leave dictionaries it began
                    if i.ds.len() > base_ds {
                        i.ds.truncate(base_ds.max(3));
                    }
                    i.push(Obj::Bool(true))
                }
                Err(Flow::Exit) => i.push(Obj::Bool(false)),
                Err(f) => return Err(f),
            }
            Ok(())
        }),
        op("quit", |_| Err(Flow::Quit)),
        op("countexecstack", |i| {
            i.push(Obj::Int(i.depth as i64 + 1));
            Ok(())
        }),
        op("execstack", |i| {
            let a = i.pop_arr()?;
            let s = a.sub(0, 0).unwrap_or(a);
            i.push(Obj::Array(s, false));
            Ok(())
        }),
        // ------------------------------------------------ types and attributes
        op("type", |i| {
            let o = i.pop()?;
            let n = i.intern(o.type_name());
            i.push(Obj::Name(n, true));
            Ok(())
        }),
        op("cvlit", |i| {
            let o = i.pop()?;
            i.push(o.with_exec(false));
            Ok(())
        }),
        op("cvx", |i| {
            let o = i.pop()?;
            i.push(o.with_exec(true));
            Ok(())
        }),
        op("xcheck", |i| {
            let o = i.pop()?;
            i.push(Obj::Bool(o.is_exec()));
            Ok(())
        }),
        op("executeonly", |_| Ok(())),
        op("noaccess", |_| Ok(())),
        op("readonly", |_| Ok(())),
        op("rcheck", |i| {
            i.pop()?;
            i.push(Obj::Bool(true));
            Ok(())
        }),
        op("wcheck", |i| {
            i.pop()?;
            i.push(Obj::Bool(true));
            Ok(())
        }),
        op("cvn", |i| {
            let o = i.pop()?;
            match o {
                Obj::Str(s, x) => {
                    let n = i.intern_bytes(&s.bytes());
                    i.push(Obj::Name(n, x));
                }
                n @ Obj::Name(..) => i.push(n),
                _ => return Err(PsErr::TypeCheck.into()),
            }
            Ok(())
        }),
        op("cvs", |i| {
            let s = i.pop_str()?;
            let o = i.pop()?;
            let t = i.to_text(&o);
            put_text(i, s, t.as_bytes())
        }),
        op("cvrs", |i| {
            let s = i.pop_str()?;
            let radix = i.pop_int()?;
            let n = i.pop_num()?;
            let t = if radix == 10 {
                let o = if n.fract() == 0.0 { Obj::Int(n as i64) } else { Obj::Real(n) };
                i.to_text(&o)
            } else {
                let v = n as i64 as i32 as u32;
                to_radix(v, radix.clamp(2, 36) as u32)
            };
            put_text(i, s, t.as_bytes())
        }),
        // ------------------------------------------------ dictionaries
        op("dict", |i| {
            let n = i.pop_usize()?;
            i.push(Obj::Dict(new_dict(n)));
            Ok(())
        }),
        op("maxlength", |i| {
            let d = i.pop_dict()?;
            let n = { d.borrow().cap.max(d.borrow().map.len() + 1) };
            i.push(Obj::Int(n as i64));
            Ok(())
        }),
        op("begin", |i| {
            let d = i.pop_dict()?;
            if i.ds.len() > 500 {
                return Err(PsErr::LimitCheck.into());
            }
            i.ds.push(d);
            Ok(())
        }),
        op("end", |i| {
            if i.ds.len() <= 3 {
                return Err(PsErr::DictStackUnderflow.into());
            }
            i.ds.pop();
            Ok(())
        }),
        op("def", |i| {
            let v = i.pop()?;
            let k = i.pop()?;
            let k = i.to_key(&k)?;
            i.def(k, v);
            Ok(())
        }),
        op("load", |i| {
            let k = i.pop()?;
            let key = i.to_key(&k)?;
            let found = i.ds.iter().rev().find_map(|d| d.borrow().map.get(&key).cloned());
            let found = found.or_else(|| i.fallback.borrow().map.get(&key).cloned());
            match found {
                Some(v) => {
                    i.push(v);
                    Ok(())
                }
                None => {
                    i.push(k);
                    Err(PsErr::Undefined.into())
                }
            }
        }),
        op("store", |i| {
            let v = i.pop()?;
            let k = i.pop()?;
            let key = i.to_key(&k)?;
            let d = i.ds.iter().rev().find(|d| d.borrow().map.contains_key(&key)).cloned();
            match d {
                Some(d) => {
                    d.borrow_mut().map.insert(key, v);
                }
                None => i.def(key, v),
            }
            Ok(())
        }),
        op("get", |i| {
            let k = i.pop()?;
            let c = i.pop()?;
            let v = match &c {
                Obj::Dict(d) => {
                    let key = i.to_key(&k)?;
                    let v = d.borrow().map.get(&key).cloned();
                    match v {
                        Some(v) => v,
                        None => {
                            i.push(c);
                            i.push(k);
                            return Err(PsErr::Undefined.into());
                        }
                    }
                }
                Obj::Array(a, _) => {
                    let n = k.num().ok_or(Flow::Err(PsErr::TypeCheck))? as i64;
                    if n < 0 {
                        return Err(PsErr::RangeCheck.into());
                    }
                    a.get(n as usize).ok_or(Flow::Err(PsErr::RangeCheck))?
                }
                Obj::Str(s, _) => {
                    let n = k.num().ok_or(Flow::Err(PsErr::TypeCheck))? as i64;
                    if n < 0 {
                        return Err(PsErr::RangeCheck.into());
                    }
                    Obj::Int(s.get(n as usize).ok_or(Flow::Err(PsErr::RangeCheck))? as i64)
                }
                _ => return Err(PsErr::TypeCheck.into()),
            };
            i.push(v);
            Ok(())
        }),
        op("put", |i| {
            let v = i.pop()?;
            let k = i.pop()?;
            let c = i.pop()?;
            match &c {
                Obj::Dict(d) => {
                    let key = i.to_key(&k)?;
                    d.borrow_mut().map.insert(key, v);
                }
                Obj::Array(a, _) => {
                    let n = k.num().ok_or(Flow::Err(PsErr::TypeCheck))? as i64;
                    if n < 0 || !a.put(n as usize, v) {
                        return Err(PsErr::RangeCheck.into());
                    }
                }
                Obj::Str(s, _) => {
                    let n = k.num().ok_or(Flow::Err(PsErr::TypeCheck))? as i64;
                    let b = v.num().ok_or(Flow::Err(PsErr::TypeCheck))? as i64;
                    if n < 0 || !s.put(n as usize, b as u8) {
                        return Err(PsErr::RangeCheck.into());
                    }
                }
                _ => return Err(PsErr::TypeCheck.into()),
            }
            Ok(())
        }),
        op("known", |i| {
            let k = i.pop()?;
            let d = i.pop_dict()?;
            let key = i.to_key(&k)?;
            let r = d.borrow().map.contains_key(&key);
            i.push(Obj::Bool(r));
            Ok(())
        }),
        op("undef", |i| {
            let k = i.pop()?;
            let d = i.pop_dict()?;
            let key = i.to_key(&k)?;
            d.borrow_mut().map.remove(&key);
            Ok(())
        }),
        op("where", |i| {
            let k = i.pop()?;
            let key = i.to_key(&k)?;
            let d = i.ds.iter().rev().find(|d| d.borrow().map.contains_key(&key)).cloned();
            match d {
                Some(d) => {
                    i.push(Obj::Dict(d));
                    i.push(Obj::Bool(true));
                }
                None => i.push(Obj::Bool(false)),
            }
            Ok(())
        }),
        op("currentdict", |i| {
            let d = i.ds.last().cloned().ok_or(Flow::Err(PsErr::DictStackUnderflow))?;
            i.push(Obj::Dict(d));
            Ok(())
        }),
        op("countdictstack", |i| {
            i.push(Obj::Int(i.ds.len() as i64));
            Ok(())
        }),
        op("dictstack", |i| {
            let a = i.pop_arr()?;
            let n = i.ds.len();
            if a.len < n {
                return Err(PsErr::RangeCheck.into());
            }
            for (k, d) in i.ds.clone().into_iter().enumerate() {
                a.put(k, Obj::Dict(d));
            }
            i.push(Obj::Array(a.sub(0, n).unwrap(), false));
            Ok(())
        }),
        op("cleardictstack", |i| {
            i.ds.truncate(3);
            Ok(())
        }),
        // ------------------------------------------------ arrays and strings
        op("array", |i| {
            let n = i.pop_usize()?;
            if n > 10_000_000 {
                return Err(PsErr::LimitCheck.into());
            }
            i.push(Obj::arr(vec![Obj::Null; n]));
            Ok(())
        }),
        op("packedarray", |i| {
            let n = i.pop_usize()?;
            i.need(n)?;
            let at = i.os.len() - n;
            let items: Vec<Obj> = i.os.drain(at..).collect();
            i.push(Obj::arr(items));
            Ok(())
        }),
        op("setpacking", |i| {
            i.packing = i.pop_bool()?;
            Ok(())
        }),
        op("currentpacking", |i| {
            i.push(Obj::Bool(i.packing));
            Ok(())
        }),
        op("string", |i| {
            let n = i.pop_usize()?;
            if n > 100_000_000 {
                return Err(PsErr::LimitCheck.into());
            }
            i.push(Obj::str(vec![0; n]));
            Ok(())
        }),
        op("length", |i| {
            let o = i.pop()?;
            let n = match &o {
                Obj::Array(a, _) => a.len,
                Obj::Str(s, _) => s.len,
                Obj::Dict(d) => d.borrow().map.len(),
                Obj::Name(n, _) => i.name_str(*n).len(),
                _ => return Err(PsErr::TypeCheck.into()),
            };
            i.push(Obj::Int(n as i64));
            Ok(())
        }),
        op("getinterval", |i| {
            let n = i.pop_usize()?;
            let s = i.pop_usize()?;
            match i.pop()? {
                Obj::Array(a, x) => {
                    let sub = a.sub(s, n).ok_or(Flow::Err(PsErr::RangeCheck))?;
                    i.push(Obj::Array(sub, x));
                }
                Obj::Str(st, x) => {
                    let sub = st.sub(s, n).ok_or(Flow::Err(PsErr::RangeCheck))?;
                    i.push(Obj::Str(sub, x));
                }
                _ => return Err(PsErr::TypeCheck.into()),
            }
            Ok(())
        }),
        op("putinterval", |i| {
            let src = i.pop()?;
            let at = i.pop_usize()?;
            let dst = i.pop()?;
            match (&dst, &src) {
                (Obj::Array(d, _), Obj::Array(s, _)) => {
                    if at + s.len > d.len {
                        return Err(PsErr::RangeCheck.into());
                    }
                    for (k, o) in s.items().into_iter().enumerate() {
                        d.put(at + k, o);
                    }
                }
                (Obj::Str(d, _), Obj::Str(s, _)) => {
                    if at + s.len > d.len {
                        return Err(PsErr::RangeCheck.into());
                    }
                    for (k, b) in s.bytes().into_iter().enumerate() {
                        d.put(at + k, b);
                    }
                }
                _ => return Err(PsErr::TypeCheck.into()),
            }
            Ok(())
        }),
        op("aload", |i| {
            let a = i.pop_arr()?;
            for o in a.items() {
                i.push(o);
            }
            i.push(Obj::Array(a, false));
            Ok(())
        }),
        op("astore", |i| {
            let a = i.pop_arr()?;
            i.need(a.len)?;
            let at = i.os.len() - a.len;
            let items: Vec<Obj> = i.os.drain(at..).collect();
            for (k, o) in items.into_iter().enumerate() {
                a.put(k, o);
            }
            i.push(Obj::Array(a, false));
            Ok(())
        }),
        op("anchorsearch", |i| {
            let seek = i.pop_str()?;
            let s = i.pop_str()?;
            let (sb, kb) = (s.bytes(), seek.bytes());
            if sb.starts_with(&kb) {
                i.push(Obj::Str(s.sub(kb.len(), s.len - kb.len()).unwrap(), false));
                i.push(Obj::Str(s.sub(0, kb.len()).unwrap(), false));
                i.push(Obj::Bool(true));
            } else {
                i.push(Obj::Str(s, false));
                i.push(Obj::Bool(false));
            }
            Ok(())
        }),
        op("search", |i| {
            let seek = i.pop_str()?;
            let s = i.pop_str()?;
            let (sb, kb) = (s.bytes(), seek.bytes());
            let at = if kb.is_empty() { Some(0) } else { sb.windows(kb.len()).position(|w| w == kb.as_slice()) };
            match at {
                Some(p) => {
                    i.push(Obj::Str(s.sub(p + kb.len(), s.len - p - kb.len()).unwrap(), false));
                    i.push(Obj::Str(s.sub(p, kb.len()).unwrap(), false));
                    i.push(Obj::Str(s.sub(0, p).unwrap(), false));
                    i.push(Obj::Bool(true));
                }
                None => {
                    i.push(Obj::Str(s, false));
                    i.push(Obj::Bool(false));
                }
            }
            Ok(())
        }),
        op("token", |i| match i.pop()? {
            Obj::Str(s, _) => {
                let b = s.bytes();
                let mut pos = 0;
                match i.scan(&b, &mut pos, 0) {
                    Ok(super::interp::Scanned::Obj(o)) => {
                        // the rest starts after one white-space character
                        let rest = if pos < b.len() && b[pos].is_ascii_whitespace() { pos + 1 } else { pos };
                        i.push(Obj::Str(s.sub(rest, s.len - rest).unwrap(), false));
                        i.push(o);
                        i.push(Obj::Bool(true));
                        Ok(())
                    }
                    Ok(_) => {
                        i.push(Obj::Bool(false));
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Obj::File(f, _) => match i.next_token(&f) {
                Ok(Some(o)) => {
                    i.push(o);
                    i.push(Obj::Bool(true));
                    Ok(())
                }
                Ok(None) => {
                    i.push(Obj::Bool(false));
                    Ok(())
                }
                Err(e) => Err(e.into()),
            },
            _ => Err(PsErr::TypeCheck.into()),
        }),
        // ------------------------------------------------ files
        op("currentfile", |i| {
            let f = i.files.last().cloned().unwrap_or_else(|| PsFile::from_bytes(vec![]));
            i.push(Obj::File(f, false));
            Ok(())
        }),
        op("file", |i| {
            let _access = i.pop()?;
            let name = i.pop()?;
            let n = i.text_of(&name).unwrap_or_default();
            if n == "%stdin" {
                let f = i.files.first().cloned().unwrap_or_else(|| PsFile::from_bytes(vec![]));
                i.push(Obj::File(f, false));
                return Ok(());
            }
            if n == "%stdout" || n == "%stderr" || n == "%lineedit" || n == "%statementedit" {
                i.push(Obj::File(PsFile::from_bytes(vec![]), false));
                return Ok(());
            }
            i.counts.files += 1;
            Err(PsErr::IoError.into())
        }),
        op("run", |i| {
            i.pop()?;
            i.counts.files += 1;
            Ok(())
        }),
        op("deletefile", |i| i.pop().map(|_| ())),
        op("renamefile", |i| {
            i.pop()?;
            i.pop()?;
            Ok(())
        }),
        op("status", |i| {
            match i.pop()? {
                Obj::File(f, _) => {
                    let open = !f.borrow().closed;
                    i.push(Obj::Bool(open));
                }
                _ => i.push(Obj::Bool(false)),
            }
            Ok(())
        }),
        op("closefile", |i| {
            let f = pop_file(i)?;
            f.borrow_mut().closed = true;
            Ok(())
        }),
        op("flushfile", |i| {
            let f = pop_file(i)?;
            // reading to the end of an input file
            super::filters::ensure(&f);
            let mut b = f.borrow_mut();
            if !i.files.iter().any(|x| Rc::ptr_eq(x, &f)) {
                b.pos = b.data.len();
            }
            Ok(())
        }),
        op("resetfile", |i| pop_file(i).map(|_| ())),
        op("bytesavailable", |i| {
            let f = pop_file(i)?;
            super::filters::ensure(&f);
            let b = f.borrow();
            i.push(Obj::Int(b.data.len().saturating_sub(b.pos) as i64));
            Ok(())
        }),
        op("fileposition", |i| {
            let f = pop_file(i)?;
            let p = f.borrow().pos;
            i.push(Obj::Int(p as i64));
            Ok(())
        }),
        op("setfileposition", |i| {
            let p = i.pop_usize()?;
            let f = pop_file(i)?;
            f.borrow_mut().pos = p;
            Ok(())
        }),
        op("read", |i| {
            let f = pop_file(i)?;
            super::filters::ensure(&f);
            let mut b = f.borrow_mut();
            if b.pos < b.data.len() && !b.closed {
                let c = b.data[b.pos];
                b.pos += 1;
                drop(b);
                i.push(Obj::Int(c as i64));
                i.push(Obj::Bool(true));
            } else {
                drop(b);
                i.push(Obj::Bool(false));
            }
            Ok(())
        }),
        op("readhexstring", |i| {
            let s = i.pop_str()?;
            let f = pop_file(i)?;
            super::filters::ensure(&f);
            let mut b = f.borrow_mut();
            let mut n = 0;
            let mut hi: Option<u8> = None;
            let mut buf = s.buf.borrow_mut();
            while n < s.len && b.pos < b.data.len() {
                let c = b.data[b.pos];
                b.pos += 1;
                let v = match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => continue,
                };
                match hi.take() {
                    None => hi = Some(v),
                    Some(h) => {
                        buf[s.start + n] = h << 4 | v;
                        n += 1;
                    }
                }
            }
            drop(buf);
            drop(b);
            let full = n == s.len;
            i.push(Obj::Str(s.sub(0, n).unwrap(), false));
            i.push(Obj::Bool(full));
            Ok(())
        }),
        op("readstring", |i| {
            let s = i.pop_str()?;
            let f = pop_file(i)?;
            super::filters::ensure(&f);
            let mut b = f.borrow_mut();
            let avail = b.data.len().saturating_sub(b.pos);
            let n = s.len.min(avail);
            {
                let mut buf = s.buf.borrow_mut();
                buf[s.start..s.start + n].copy_from_slice(&b.data[b.pos..b.pos + n]);
            }
            b.pos += n;
            drop(b);
            let full = n == s.len;
            i.push(Obj::Str(s.sub(0, n).unwrap(), false));
            i.push(Obj::Bool(full));
            Ok(())
        }),
        op("readline", |i| {
            let s = i.pop_str()?;
            let f = pop_file(i)?;
            super::filters::ensure(&f);
            let mut b = f.borrow_mut();
            let mut line = vec![];
            let mut found = false;
            while b.pos < b.data.len() {
                let c = b.data[b.pos];
                b.pos += 1;
                if c == b'\n' {
                    found = true;
                    break;
                }
                if c == b'\r' {
                    if b.pos < b.data.len() && b.data[b.pos] == b'\n' {
                        b.pos += 1;
                    }
                    found = true;
                    break;
                }
                line.push(c);
            }
            drop(b);
            let n = line.len().min(s.len);
            {
                let mut buf = s.buf.borrow_mut();
                buf[s.start..s.start + n].copy_from_slice(&line[..n]);
            }
            i.push(Obj::Str(s.sub(0, n).unwrap(), false));
            i.push(Obj::Bool(found || n > 0));
            Ok(())
        }),
        op("write", |i| {
            i.pop()?;
            i.pop()?;
            Ok(())
        }),
        op("writestring", |i| {
            i.pop()?;
            i.pop()?;
            Ok(())
        }),
        op("writehexstring", |i| {
            i.pop()?;
            i.pop()?;
            Ok(())
        }),
        op("print", |i| i.pop().map(|_| ())),
        op("=", |i| i.pop().map(|_| ())),
        op("==", |i| i.pop().map(|_| ())),
        op("stack", |_| Ok(())),
        op("pstack", |_| Ok(())),
        op("flush", |_| Ok(())),
        op("echo", |i| i.pop().map(|_| ())),
        op("prompt", |_| Ok(())),
        op("filter", filter),
        op("eexec", |i| {
            let src = i.pop()?;
            let f = match src {
                Obj::File(f, _) => f,
                Obj::Str(s, _) => PsFile::from_bytes(s.bytes()),
                _ => return Err(PsErr::TypeCheck.into()),
            };
            super::filters::ensure(&f);
            let (data, pos) = {
                let b = f.borrow();
                (b.data.clone(), b.pos)
            };
            let rest = &data[pos.min(data.len())..];
            let end = super::filters::eexec_end(rest);
            let plain = super::filters::eexec_decrypt(&rest[..end]);
            f.borrow_mut().pos = pos + end;
            let sys = i.systemdict.clone();
            i.ds.push(sys.clone());
            let r = i.run_file(PsFile::from_bytes(plain));
            if i.ds.len() > 3 && i.ds.last().is_some_and(|d| Rc::ptr_eq(d, &sys)) {
                i.ds.pop();
            }
            r
        }),
        // ------------------------------------------------ VM
        op("save", |i| {
            i.gsave();
            i.saves.push(i.gstack.len());
            i.push(Obj::Save(i.saves.len() - 1));
            Ok(())
        }),
        op("restore", |i| {
            let s = match i.pop()? {
                Obj::Save(s) => s,
                _ => return Err(PsErr::TypeCheck.into()),
            };
            if s >= i.saves.len() {
                return Ok(());
            }
            let depth = i.saves[s];
            i.saves.truncate(s);
            i.gstack.truncate(depth);
            if let Some(g) = i.gstack.pop() {
                i.gs = g;
            }
            Ok(())
        }),
        op("vmstatus", |i| {
            i.push(Obj::Int(i.saves.len() as i64));
            i.push(Obj::Int(100_000));
            i.push(Obj::Int(100_000_000));
            Ok(())
        }),
        op("vmreclaim", |i| i.pop().map(|_| ())),
        op("setvmthreshold", |i| i.pop().map(|_| ())),
        op("setglobal", |i| i.pop().map(|_| ())),
        op("currentglobal", |i| {
            i.push(Obj::Bool(false));
            Ok(())
        }),
        op("gcheck", |i| {
            i.pop()?;
            i.push(Obj::Bool(false));
            Ok(())
        }),
        op("bind", |i| {
            let p = i.pop()?;
            if let Obj::Array(a, _) = &p {
                let mut seen = std::collections::HashSet::new();
                bind(i, a, &mut seen, 0);
            }
            i.push(p);
            Ok(())
        }),
        // ------------------------------------------------ interpreter and device
        op("version", |i| {
            i.push(Obj::str(b"3011".to_vec()));
            Ok(())
        }),
        op("languagelevel", |i| {
            i.push(Obj::Int(3));
            Ok(())
        }),
        op("product", |i| {
            i.push(Obj::str(b"AnyVector PostScript Import".to_vec()));
            Ok(())
        }),
        op("revision", |i| {
            i.push(Obj::Int(1));
            Ok(())
        }),
        op("serialnumber", |i| {
            i.push(Obj::Int(0));
            Ok(())
        }),
        op("realtime", |i| {
            i.push(Obj::Int(i.started.elapsed().as_millis() as i64));
            Ok(())
        }),
        op("usertime", |i| {
            i.push(Obj::Int(i.started.elapsed().as_millis() as i64));
            Ok(())
        }),
        op("setuserparams", |i| i.pop().map(|_| ())),
        op("setsystemparams", |i| i.pop().map(|_| ())),
        op("setdevparams", |i| {
            i.pop()?;
            i.pop()?;
            Ok(())
        }),
        op("currentuserparams", |i| {
            i.push(Obj::Dict(new_dict(4)));
            Ok(())
        }),
        op("currentsystemparams", |i| {
            i.push(Obj::Dict(new_dict(4)));
            Ok(())
        }),
        op("currentdevparams", |i| {
            i.pop()?;
            i.push(Obj::Dict(new_dict(4)));
            Ok(())
        }),
        op("setucacheparams", |i| {
            let m = mark_pos(i)?;
            i.os.truncate(m);
            Ok(())
        }),
        op("ucachestatus", |i| {
            i.push(Obj::Mark);
            for _ in 0..5 {
                i.push(Obj::Int(0));
            }
            Ok(())
        }),
        op("setcachelimit", |i| i.pop().map(|_| ())),
        op("setcacheparams", |i| {
            let m = mark_pos(i)?;
            i.os.truncate(m);
            Ok(())
        }),
        op("currentcacheparams", |i| {
            i.push(Obj::Mark);
            i.push(Obj::Int(100000));
            i.push(Obj::Int(1000));
            Ok(())
        }),
        op("cachestatus", |i| {
            for _ in 0..7 {
                i.push(Obj::Int(0));
            }
            Ok(())
        }),
        op("internaldict", |i| {
            i.pop()?;
            let d = match i.dget(&i.systemdict.clone(), "AVInternal") {
                Some(Obj::Dict(d)) => d,
                _ => new_dict(8),
            };
            i.push(Obj::Dict(d));
            Ok(())
        }),
        op("startjob", |i| {
            i.pop()?;
            i.pop()?;
            i.push(Obj::Bool(false));
            Ok(())
        }),
        op("exitserver", |i| i.pop().map(|_| ())),
        op("setpagedevice", |i| {
            let d = i.pop_dict()?;
            if let Some(Obj::Array(a, _)) = i.dget(&d, "PageSize") {
                if let (Some(w), Some(h)) = (a.get(0).and_then(|o| o.num()), a.get(1).and_then(|o| o.num())) {
                    if w > 0.0 && h > 0.0 && i.out.pages.is_empty() {
                        i.set_page_size(w, h);
                    }
                }
            }
            Ok(())
        }),
        op("currentpagedevice", |i| {
            let d = new_dict(8);
            let (w, h) = i.page_size.unwrap_or((612.0, 792.0));
            i.dput(&d, "PageSize", Obj::arr(vec![Obj::Real(w), Obj::Real(h)]));
            i.dput(&d, "HWResolution", Obj::arr(vec![Obj::Real(72.0), Obj::Real(72.0)]));
            i.dput(&d, "Policies", Obj::Dict(new_dict(4)));
            i.dput(&d, "Orientation", Obj::Int(0));
            let pcm = i.lit("DeviceCMYK");
            i.dput(&d, "ProcessColorModel", pcm);
            i.push(Obj::Dict(d));
            Ok(())
        }),
        op("showpage", |i| {
            i.counts.pages += 1;
            if i.out.has_content() {
                i.out.finish_page();
            }
            let font = i.gs.font.clone();
            i.gs = super::gfx::GState::new();
            i.gs.font = font;
            Ok(())
        }),
        op("copypage", |_| Ok(())),
        op("handleerror", |_| Ok(())),
        op("defineuserobject", |i| {
            let v = i.pop()?;
            let n = i.pop_usize()?;
            let ud = i.userdict.clone();
            let arr = match i.dget(&ud, "UserObjects") {
                Some(Obj::Array(a, _)) if a.len > n => a,
                Some(Obj::Array(a, _)) => {
                    let mut items = a.items();
                    items.resize(n + 1, Obj::Null);
                    PsArr::new(items)
                }
                _ => PsArr::new(vec![Obj::Null; n + 1]),
            };
            arr.put(n, v);
            i.dput(&ud, "UserObjects", Obj::Array(arr, false));
            Ok(())
        }),
        op("execuserobject", |i| {
            let n = i.pop_usize()?;
            let ud = i.userdict.clone();
            match i.dget(&ud, "UserObjects") {
                Some(Obj::Array(a, _)) => {
                    let v = a.get(n).ok_or(Flow::Err(PsErr::RangeCheck))?;
                    i.exec_value(v)
                }
                _ => Err(PsErr::Undefined.into()),
            }
        }),
        op("undefineuserobject", |i| i.pop().map(|_| ())),
        // ------------------------------------------------ resources
        op("findresource", |i| {
            let cat = i.pop()?;
            let key = i.pop()?;
            let cat = i.text_of(&cat).unwrap_or_default();
            if cat == "Font" || cat == "CIDFont" {
                i.push(key);
                return super::text::findfont(i);
            }
            match i.find_resource(&cat, &key)? {
                Some(v) => {
                    i.push(v);
                    Ok(())
                }
                None => Err(PsErr::Undefined.into()),
            }
        }),
        op("findencoding", |i| {
            let key = i.pop()?;
            match i.find_resource("Encoding", &key)? {
                Some(v) => {
                    i.push(v);
                    Ok(())
                }
                None => Err(PsErr::Undefined.into()),
            }
        }),
        op("defineresource", |i| {
            let cat = i.pop()?;
            let inst = i.pop()?;
            let key = i.pop()?;
            let cat = i.text_of(&cat).unwrap_or_default();
            if cat == "Font" {
                i.push(key);
                i.push(inst);
                return super::text::definefont(i);
            }
            let d = i.category(&cat);
            let k = i.to_key(&key)?;
            d.borrow_mut().map.insert(k, inst.clone());
            i.push(inst);
            Ok(())
        }),
        op("undefineresource", |i| {
            let cat = i.pop()?;
            let key = i.pop()?;
            let cat = i.text_of(&cat).unwrap_or_default();
            let d = if cat == "Font" { i.fontdir.clone() } else { i.category(&cat) };
            let k = i.to_key(&key)?;
            d.borrow_mut().map.remove(&k);
            Ok(())
        }),
        op("resourcestatus", |i| {
            let cat = i.pop()?;
            let key = i.pop()?;
            let cat = i.text_of(&cat).unwrap_or_default();
            let found = if cat == "Font" {
                let k = i.to_key(&key)?;
                i.fontdir.borrow().map.contains_key(&k)
            } else {
                i.find_resource(&cat, &key)?.is_some()
            };
            if found {
                i.push(Obj::Int(1));
                i.push(Obj::Int(0));
                i.push(Obj::Bool(true));
            } else {
                i.push(Obj::Bool(false));
            }
            Ok(())
        }),
        op("resourceforall", |i| {
            let cat = i.pop()?;
            let _scratch = i.pop_str()?;
            let p = i.pop_proc()?.with_exec(true);
            let template = i.pop()?;
            let cat = i.text_of(&cat).unwrap_or_default();
            let t = i.text_of(&template).unwrap_or_default();
            let d = if cat == "Font" { i.fontdir.clone() } else { i.category(&cat) };
            let keys: Vec<Key> = d.borrow().map.keys().cloned().collect();
            for k in keys {
                let ko = i.key_obj(&k);
                let Some(name) = i.text_of(&ko) else { continue };
                if glob(t.as_bytes(), name.as_bytes()) {
                    i.push(Obj::str(name.into_bytes()));
                    match i.exec_value(p.clone()) {
                        Err(Flow::Exit) => break,
                        r => r?,
                    }
                }
            }
            Ok(())
        }),
        op("execform", |i| {
            let d = i.pop_dict()?;
            let m = match i.dget(&d, "Matrix") {
                Some(Obj::Array(a, _)) => super::gfx::mat_read(&a).unwrap_or(kurbo::Affine::IDENTITY),
                _ => kurbo::Affine::IDENTITY,
            };
            let paint = i.dget(&d, "PaintProc").ok_or(Flow::Err(PsErr::Undefined))?;
            i.gsave();
            i.gs.ctm *= m;
            if let Some(Obj::Array(b, _)) = i.dget(&d, "BBox") {
                let v: Vec<f64> = b.items().iter().filter_map(|o| o.num()).collect();
                if v.len() == 4 {
                    let ctm = i.gs.ctm;
                    let r = kurbo::Rect::new(v[0], v[1], v[2], v[3]);
                    let path = ctm * super::gfx::rect_path(r);
                    i.gs.newpath();
                    i.gs.path = path;
                    i.clip(false);
                    i.gs.newpath();
                }
            }
            i.push(Obj::Dict(d));
            let r = i.exec_value(paint.with_exec(true));
            i.grestore();
            r
        }),
    ]
}

fn mark_pos(i: &Interp) -> R<usize> {
    i.os.iter().rposition(|o| matches!(o, Obj::Mark)).ok_or(Flow::Err(PsErr::Unregistered))
}

fn pop_file(i: &mut Interp) -> R<FileRef> {
    match i.pop()? {
        Obj::File(f, _) => Ok(f),
        _ => Err(PsErr::TypeCheck.into()),
    }
}

fn copy(i: &mut Interp) -> R {
    let top = i.pop()?;
    match top {
        Obj::Int(n) => {
            if n < 0 || n as usize > i.os.len() {
                return Err(PsErr::RangeCheck.into());
            }
            let at = i.os.len() - n as usize;
            let v: Vec<Obj> = i.os[at..].to_vec();
            i.os.extend(v);
            Ok(())
        }
        Obj::Array(dst, x) => {
            let src = i.pop_arr()?;
            if src.len > dst.len {
                return Err(PsErr::RangeCheck.into());
            }
            for (k, o) in src.items().into_iter().enumerate() {
                dst.put(k, o);
            }
            i.push(Obj::Array(dst.sub(0, src.len).unwrap(), x));
            Ok(())
        }
        Obj::Str(dst, x) => {
            let src = i.pop_str()?;
            if src.len > dst.len {
                return Err(PsErr::RangeCheck.into());
            }
            for (k, b) in src.bytes().into_iter().enumerate() {
                dst.put(k, b);
            }
            i.push(Obj::Str(dst.sub(0, src.len).unwrap(), x));
            Ok(())
        }
        Obj::Dict(dst) => {
            let src = i.pop_dict()?;
            let pairs: Vec<(Key, Obj)> = src.borrow().map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (k, v) in pairs {
                dst.borrow_mut().map.insert(k, v);
            }
            i.push(Obj::Dict(dst));
            Ok(())
        }
        _ => Err(PsErr::TypeCheck.into()),
    }
}

fn arith(i: &mut Interp, int: fn(i64, i64) -> Option<i64>, real: fn(f64, f64) -> f64) -> R {
    i.need(2)?;
    let n = i.os.len();
    let r = match (&i.os[n - 2], &i.os[n - 1]) {
        (Obj::Int(a), Obj::Int(b)) => match int(*a, *b) {
            Some(v) => Obj::int_or_real(v as i128),
            None => Obj::Real(real(*a as f64, *b as f64)),
        },
        (a, b) => match (a.num(), b.num()) {
            (Some(a), Some(b)) => Obj::Real(real(a, b)),
            _ => return Err(PsErr::TypeCheck.into()),
        },
    };
    i.os.truncate(n - 2);
    i.push(r);
    Ok(())
}

fn unary(i: &mut Interp, int: fn(i64) -> Option<i64>, real: fn(f64) -> f64) -> R {
    let o = i.pop()?;
    let r = match o {
        Obj::Int(a) => match int(a) {
            Some(v) => Obj::int_or_real(v as i128),
            None => Obj::Real(real(a as f64)),
        },
        Obj::Real(a) => Obj::Real(real(a)),
        o => {
            i.push(o);
            return Err(PsErr::TypeCheck.into());
        }
    };
    i.push(r);
    Ok(())
}

fn compare(i: &mut Interp, f: fn(std::cmp::Ordering) -> bool) -> R {
    let b = i.pop()?;
    let a = i.pop()?;
    let ord = match (&a, &b) {
        (Obj::Str(x, _), Obj::Str(y, _)) => x.bytes().cmp(&y.bytes()),
        _ => match (a.num(), b.num()) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => return Err(PsErr::TypeCheck.into()),
        },
    };
    i.push(Obj::Bool(f(ord)));
    Ok(())
}

fn logic(i: &mut Interp, int: fn(i64, i64) -> i64, b: fn(bool, bool) -> bool) -> R {
    let y = i.pop()?;
    let x = i.pop()?;
    match (x, y) {
        (Obj::Bool(p), Obj::Bool(q)) => i.push(Obj::Bool(b(p, q))),
        (Obj::Int(p), Obj::Int(q)) => i.push(Obj::Int(int(p, q))),
        _ => return Err(PsErr::TypeCheck.into()),
    }
    Ok(())
}

fn put_text(i: &mut Interp, s: PsStr, t: &[u8]) -> R {
    if t.len() > s.len {
        return Err(PsErr::RangeCheck.into());
    }
    for (k, b) in t.iter().enumerate() {
        s.put(k, *b);
    }
    i.push(Obj::Str(s.sub(0, t.len()).unwrap(), false));
    Ok(())
}

fn to_radix(mut v: u32, r: u32) -> String {
    if v == 0 {
        return "0".into();
    }
    let mut out = vec![];
    while v > 0 {
        out.push(std::char::from_digit(v % r, r).unwrap().to_ascii_uppercase());
        v /= r;
    }
    out.iter().rev().collect()
}

/// Whether a name matches a resource template (`*` any run, `?` one character).
fn glob(t: &[u8], s: &[u8]) -> bool {
    match (t.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&t[1..], s) || (!s.is_empty() && glob(t, &s[1..])),
        (Some(b'?'), Some(_)) => glob(&t[1..], &s[1..]),
        (Some(a), Some(b)) if a == b => glob(&t[1..], &s[1..]),
        _ => false,
    }
}

/// `bind`: names that are operators become the operators, in nested procedures too.
fn bind(i: &mut Interp, a: &PsArr, seen: &mut std::collections::HashSet<usize>, depth: usize) {
    let id = Rc::as_ptr(&a.buf) as *const u8 as usize + a.start;
    if depth > 100 || !seen.insert(id) {
        return;
    }
    for k in 0..a.len {
        match a.get(k) {
            Some(Obj::Name(n, true)) => {
                if let Some(Obj::Op(o)) = i.lookup(n) {
                    a.put(k, Obj::Op(o));
                }
            }
            Some(Obj::Array(sub, true)) => bind(i, &sub, seen, depth + 1),
            _ => {}
        }
    }
}

/// `filter`: decoding filters (encoding filters hand back an empty file).
fn filter(i: &mut Interp) -> R {
    let name = i.pop()?;
    let name = i.text_of(&name).ok_or(Flow::Err(PsErr::TypeCheck))?;
    let mut p = super::filters::Params::default();
    let mut dict: Option<DictRef> = None;
    if let Some(Obj::Dict(d)) = i.os.last() {
        dict = Some(d.clone());
        i.pop()?;
    }
    if name == "SubFileDecode" && dict.is_none() {
        let s = i.pop()?;
        p.eod_string = match s {
            Obj::Str(s, _) => s.bytes(),
            _ => vec![],
        };
        p.eod_count = i.pop_int()?;
    }
    if let Some(d) = &dict {
        let g = |i: &mut Interp, k: &str| i.dnum(d, k);
        if let Some(v) = g(i, "Predictor") {
            p.predictor = v as i64;
        }
        if let Some(v) = g(i, "Colors") {
            p.colors = v.max(1.0) as usize;
        }
        if let Some(v) = g(i, "BitsPerComponent") {
            p.bpc = v.max(1.0) as usize;
        }
        if let Some(v) = g(i, "Columns") {
            p.columns = v.max(1.0) as usize;
        }
        if let Some(v) = g(i, "EarlyChange") {
            p.early_change = v as i64;
        }
        if let Some(v) = g(i, "EODCount") {
            p.eod_count = v as i64;
        }
        if let Some(Obj::Str(s, _)) = i.dget(d, "EODString") {
            p.eod_string = s.bytes();
        }
    }
    let src = i.pop()?;
    let source = match src {
        Obj::File(f, _) => FilterSource::File(f),
        Obj::Str(s, _) => FilterSource::Bytes(s.bytes()),
        // a procedure giving the data: run it until it gives an empty string
        p @ Obj::Array(..) => {
            let mut all = vec![];
            for _ in 0..100_000 {
                i.exec_value(p.clone().with_exec(true))?;
                match i.pop()? {
                    Obj::Str(s, _) if s.len > 0 => all.extend(s.bytes()),
                    _ => break,
                }
            }
            FilterSource::Bytes(all)
        }
        _ => return Err(PsErr::TypeCheck.into()),
    };
    if name.ends_with("Encode") {
        i.push(Obj::File(PsFile::from_bytes(vec![]), false));
        return Ok(());
    }
    if !super::filters::known(&name) {
        // CCITTFax, JBIG2, JPX...: no data
        let f = PsFile::filter(Filter { name: "SubFileDecode".into(), params: super::filters::Params { eod_count: 0, ..Default::default() }, source });
        i.push(Obj::File(f, false));
        return Ok(());
    }
    let f = PsFile::filter(Filter { name: name.clone(), params: p, source });
    // ReusableStreamDecode reads its source now
    if name == "ReusableStreamDecode" {
        super::filters::ensure(&f);
    }
    i.push(Obj::File(f, false));
    Ok(())
}

/// Numbers in the encoded number string form (`rectfill` with a string): header 149, then
/// the representation byte and count.
pub fn decode_number_string(b: &[u8]) -> Vec<f64> {
    if b.len() < 4 || b[0] != 149 {
        return vec![];
    }
    let r = b[1];
    let be = r < 128;
    let n = if be { u16::from_be_bytes([b[2], b[3]]) } else { u16::from_le_bytes([b[2], b[3]]) } as usize;
    let mut out = vec![];
    let mut at = 4;
    let rr = r % 128;
    for _ in 0..n {
        let v = match rr {
            0..=31 => {
                let Some(s) = b.get(at..at + 4) else { break };
                at += 4;
                let v = if be { i32::from_be_bytes([s[0], s[1], s[2], s[3]]) } else { i32::from_le_bytes([s[0], s[1], s[2], s[3]]) };
                v as f64 / (1u64 << rr) as f64
            }
            32..=47 => {
                let Some(s) = b.get(at..at + 2) else { break };
                at += 2;
                let v = if be { i16::from_be_bytes([s[0], s[1]]) } else { i16::from_le_bytes([s[0], s[1]]) };
                v as f64 / (1u64 << (rr - 32)) as f64
            }
            48 | 49 => {
                let Some(s) = b.get(at..at + 4) else { break };
                at += 4;
                (if be { f32::from_be_bytes([s[0], s[1], s[2], s[3]]) } else { f32::from_le_bytes([s[0], s[1], s[2], s[3]]) }) as f64
            }
            _ => break,
        };
        out.push(v);
    }
    out
}

impl Interp {
    pub fn eq(&self, a: &Obj, b: &Obj) -> bool {
        match (a, b) {
            (Obj::Name(x, _), Obj::Name(y, _)) => x == y,
            (Obj::Str(x, _), Obj::Str(y, _)) => x.bytes() == y.bytes(),
            (Obj::Name(n, _), Obj::Str(s, _)) | (Obj::Str(s, _), Obj::Name(n, _)) => self.name_str(*n).as_bytes() == s.bytes().as_slice(),
            (Obj::Bool(x), Obj::Bool(y)) => x == y,
            (Obj::Null, Obj::Null) | (Obj::Mark, Obj::Mark) => true,
            (Obj::Array(x, _), Obj::Array(y, _)) => x.same(y),
            (Obj::Dict(x), Obj::Dict(y)) => Rc::ptr_eq(x, y),
            (Obj::File(x, _), Obj::File(y, _)) => Rc::ptr_eq(x, y),
            (Obj::Op(x), Obj::Op(y)) => x == y,
            (Obj::Save(x), Obj::Save(y)) => x == y,
            (Obj::FontId(x), Obj::FontId(y)) => x == y,
            _ => match (a.num(), b.num()) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            },
        }
    }

    pub fn to_text(&self, o: &Obj) -> String {
        match o {
            Obj::Int(v) => v.to_string(),
            Obj::Real(v) => fmt_real(*v),
            Obj::Bool(b) => b.to_string(),
            Obj::Name(n, _) => self.name_str(*n).to_string(),
            Obj::Str(s, _) => s.text(),
            Obj::Op(k) => self.ops[*k as usize].0.to_string(),
            _ => "--nostringval--".into(),
        }
    }

    /// The dictionary of a resource category.
    pub fn category(&mut self, cat: &str) -> DictRef {
        let res = self.resources.clone();
        if let Some(Obj::Dict(d)) = self.dget(&res, cat) {
            return d;
        }
        let d = new_dict(16);
        self.dput(&res, cat, Obj::Dict(d.clone()));
        d
    }

    pub fn find_resource(&mut self, cat: &str, key: &Obj) -> R<Option<Obj>> {
        let k = self.to_key(key)?;
        let d = self.category(cat);
        if let Some(v) = d.borrow().map.get(&k).cloned() {
            return Ok(Some(v));
        }
        if cat == "Encoding" {
            let name = self.text_of(key).unwrap_or_default();
            if let Some(v) = self.dget(&self.systemdict.clone(), &name) {
                return Ok(Some(v));
            }
        }
        if cat == "Category" || cat == "ColorSpaceFamily" || cat == "Filter" {
            // categories and families this interpreter knows
            let d = new_dict(4);
            return Ok(Some(Obj::Dict(d)));
        }
        Ok(None)
    }
}

/// A real as PostScript prints it.
pub fn fmt_real(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e7 {
        format!("{v:.1}")
    } else {
        let s = format!("{}", (v * 1e6).round() / 1e6);
        if s.contains('.') || s.contains('e') {
            s
        } else {
            format!("{s}.0")
        }
    }
}
