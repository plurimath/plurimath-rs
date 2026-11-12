//! Canonical numeric JSON and optional exact quantity evaluation.
mod decimal_math;
mod gum;
#[cfg(feature = "quantity")]
mod quantity;
#[cfg(feature = "quantity")]
mod quantity_magnitude;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational as Q;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{json, Value as J};
use std::collections::BTreeMap;
const QUANTITY_SCHEMA: &str = "plurimath.evaluate.quantity/1";
const SCHEMA: &str = "plurimath.evaluate.numeric/1";
type E = &'static str;
#[derive(Clone)]
enum N {
    Exact(Q, bool),
    Float(f64),
    Decimal(decimal_math::D),
}
fn pow10(n: u32) -> BigInt {
    BigInt::from(10u8).pow(n)
}
fn digits(n: &BigInt) -> i64 {
    n.abs().to_string().len() as i64
}
fn decimal(s: &str) -> bool {
    let mut s = s;
    if s.starts_with(['+', '-']) {
        s = &s[1..];
    }
    let parts: Vec<_> = s.split(['e', 'E']).collect();
    if parts.len() > 2 {
        return false;
    }
    if parts.len() == 2 {
        let e = parts[1].trim_start_matches(['+', '-']);
        if e.is_empty()
            || !e.bytes().all(|c| c.is_ascii_digit())
            || parts[1].chars().skip(1).any(|c| c == '+' || c == '-')
        {
            return false;
        }
    }
    let mut dot = false;
    let mut count = 0;
    for c in parts[0].bytes() {
        if c == b'.' && !dot {
            dot = true
        } else if c.is_ascii_digit() {
            count += 1
        } else {
            return false;
        }
    }
    count > 0
}
fn name(s: &str) -> bool {
    let mut b = s.bytes();
    matches!(b.next(),Some(c) if c.is_ascii_alphabetic()||c==b'_')
        && b.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn binding_decimal(s: &str) -> bool {
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    let mut pieces = unsigned.split('.');
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit());
    pieces.next().is_some_and(digits) && pieces.next().is_none_or(digits) && pieces.next().is_none()
}
fn qparse(s: &str) -> Result<Q, E> {
    let parts: Vec<_> = s.split(['e', 'E']).collect();
    let exponent: i64 = if parts.len() == 2 {
        parts[1].parse().map_err(|_| "UnsupportedExpressionError")?
    } else {
        0
    };
    let after = parts[0].split('.').nth(1).map_or(0, |x| x.len() as i64);
    let coeff = parts[0]
        .replace('.', "")
        .parse::<BigInt>()
        .map_err(|_| "InvalidRequest")?;
    let shift = exponent - after;
    if shift.unsigned_abs() > 1_000_000 {
        return Err("UnsupportedExpressionError");
    };
    Ok(if shift >= 0 {
        Q::from_integer(coeff * pow10(shift as u32))
    } else {
        Q::new(coeff, pow10((-shift) as u32))
    })
}
fn keys(v: &J, allowed: &[&str], required: &[&str]) -> Result<(), E> {
    let m = v.as_object().ok_or("InvalidRequest")?;
    if m.keys().any(|k| !allowed.contains(&k.as_str()))
        || required.iter().any(|k| !m.contains_key(*k))
    {
        Err("InvalidRequest")
    } else {
        Ok(())
    }
}
fn text<'a>(v: &'a J, k: &str) -> Result<&'a str, E> {
    v[k].as_str().ok_or("InvalidRequest")
}
fn ast(v: &J, d: usize) -> Result<(), E> {
    if d > 64 {
        return Err("InvalidRequest");
    };
    match text(v, "type")? {
        "number" => {
            keys(v, &["type", "text"], &["type", "text"])?;
            if !decimal(text(v, "text")?) {
                return Err("InvalidRequest");
            }
        }
        "variable" => {
            keys(v, &["type", "name"], &["type", "name"])?;
            if !name(text(v, "name")?) {
                return Err("InvalidRequest");
            }
        }
        "unary" => {
            keys(
                v,
                &["type", "operator", "operand"],
                &["type", "operator", "operand"],
            )?;
            if !["+", "-"].contains(&text(v, "operator")?) {
                return Err("InvalidRequest");
            };
            ast(&v["operand"], d + 1)?
        }
        "binary" => {
            keys(
                v,
                &["type", "operator", "left", "right"],
                &["type", "operator", "left", "right"],
            )?;
            if !["+", "-", "*", "/", "^"].contains(&text(v, "operator")?) {
                return Err("InvalidRequest");
            };
            ast(&v["left"], d + 1)?;
            ast(&v["right"], d + 1)?
        }
        "call" => {
            keys(
                v,
                &["type", "name", "argument"],
                &["type", "name", "argument"],
            )?;
            if !["sqrt", "sin", "cos", "exp", "ln", "abs"].contains(&text(v, "name")?) {
                return Err("InvalidRequest");
            };
            ast(&v["argument"], d + 1)?
        }
        _ => return Err("InvalidRequest"),
    };
    Ok(())
}
impl N {
    fn exact(&self) -> bool {
        matches!(self, Self::Exact(..))
    }
    fn q(&self) -> Option<Q> {
        match self {
            Self::Exact(q, _) => Some(q.clone()),
            Self::Decimal(d) => {
                if d.finite() {
                    Some(d.q())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn f(&self) -> f64 {
        match self {
            Self::Float(x) => *x,
            Self::Decimal(d) => d.f(),
            _ => self
                .q()
                .unwrap()
                .to_f64()
                .unwrap_or(if self.q().unwrap().is_negative() {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }),
        }
    }
    fn zero(&self) -> bool {
        self.q().map_or(self.f() == 0.0, |q| q.is_zero())
    }
    fn negative(&self) -> bool {
        match self {
            Self::Float(x) => x.is_sign_negative(),
            Self::Decimal(d) => d.negative(),
            _ => self.q().unwrap().is_negative(),
        }
    }
    fn wire(&self) -> J {
        match self {
            Self::Exact(q, true) => json!({"kind":"integer","value":q.numer().to_string()}),
            Self::Exact(q, false) => {
                json!({"kind":"rational","numerator":q.numer().to_string(),"denominator":q.denom().to_string()})
            }
            Self::Float(x) => json!({"kind":"float","bits":format!("{:016x}",x.to_bits())}),
            Self::Decimal(d) => d.wire(),
        }
    }
    fn neg(self) -> Self {
        match self {
            Self::Exact(q, k) => Self::Exact(-q, k),
            Self::Float(x) => Self::Float(-x),
            Self::Decimal(d) => Self::Decimal(d.neg()),
        }
    }
}
fn normalize(mut c: BigInt, mut s: i64) -> N {
    if !c.is_zero() {
        while (&c % 10u8).is_zero() {
            c /= 10u8;
            s += 1
        }
    }
    N::Decimal(decimal_math::D::new(c, s, false))
}
fn exponent(q: &Q) -> i64 {
    let mut e = digits(q.numer()) - digits(q.denom());
    let less = if e >= 0 {
        q.numer().abs() < q.denom() * pow10(e as u32)
    } else {
        q.numer().abs() * pow10((-e) as u32) < *q.denom()
    };
    if less {
        e -= 1
    };
    e
}
fn roundq(q: &Q, d: u32) -> N {
    if q.is_zero() {
        return N::Decimal(decimal_math::D::new(BigInt::zero(), 0, false));
    }
    let shift = d as i64 - 1 - exponent(q);
    let (n, den) = if shift >= 0 {
        (q.numer().abs() * pow10(shift as u32), q.denom().clone())
    } else {
        (q.numer().abs(), q.denom() * pow10((-shift) as u32))
    };
    let mut c = &n / &den;
    if (&n % &den) * 2 >= den {
        c += 1
    };
    if q.is_negative() {
        c = -c
    };
    normalize(c, -shift)
}
fn guard_sqrt(q: &Q, d: u32) -> Result<N, E> {
    if q.is_negative() {
        return Err("NonFiniteResultError");
    };
    if q.is_zero() {
        return Ok(N::Decimal(decimal_math::D::new(BigInt::zero(), 0, false)));
    };
    let e = exponent(q).div_euclid(2);
    let shift = d as i64 - 1 - e;
    let (n, den) = if shift >= 0 {
        (q.numer() * pow10((2 * shift) as u32), q.denom().clone())
    } else {
        (q.numer().clone(), q.denom() * pow10((-2 * shift) as u32))
    };
    let mut c = (&n / &den).sqrt();
    if &n * 4 >= &den * (&c * BigInt::from(2u8) + BigInt::one()).pow(2) {
        c += 1
    };
    Ok(normalize(c, -shift))
}
fn accuracy(values: &[N]) -> String {
    if values.iter().any(|n| matches!(n, N::Float(_))) {
        "limited_float".into()
    } else if let Some(p) = values
        .iter()
        .filter_map(|n| {
            if let N::Decimal(d) = n {
                Some(d.precision())
            } else {
                None
            }
        })
        .min()
    {
        format!("limited_{}", p)
    } else {
        "exact".into()
    }
}
// Ruby math.c get_double_rshift, mirrored by the pinned TS libm.ts.
fn math_log(value: &N) -> Result<f64, E> {
    if let N::Exact(q, true) = value {
        if q.is_positive() && q.numer().bits() >= 1024 {
            let shift = q.numer().bits() - 53;
            let reduced = (q.numer() >> shift as usize).to_f64().unwrap();
            return Ok(libm::log(reduced) + shift as f64 * std::f64::consts::LN_2);
        }
    }
    let x = value.f();
    if x < 0.0 {
        return Err("MathDomainError");
    }
    Ok(libm::log(x))
}
struct Eval {
    strict: bool,
    guard: Option<u32>,
    bindings: BTreeMap<String, N>,
    boundaries: Vec<J>,
}
impl Eval {
    fn crossing(&mut self, op: &str, operands: &[N], action: &str, result: Option<&N>) {
        self.boundaries.push(json!({"operation":op,"operands":operands.iter().map(N::wire).collect::<Vec<_>>(),"operand_accuracy":accuracy(operands),"action":action,"result":result.map(N::wire)}))
    }
    fn limited(&mut self, op: &str, values: &[N]) -> Result<N, E> {
        if self.strict && values.iter().all(N::exact) {
            if let Some(d) = self.guard {
                let out = if op == "sqrt" {
                    let q = if matches!(values[0], N::Exact(_, false)) {
                        roundq(&values[0].q().unwrap(), d).q().unwrap()
                    } else {
                        values[0].q().unwrap()
                    };
                    guard_sqrt(&q, d)?
                } else {
                    decimal_math::guard(op, &values[0], d)?
                };
                self.crossing(op, values, "guarded", Some(&out));
                return Ok(out);
            }
            self.crossing(op, values, "deferred", None);
            return Err("Deferred");
        }
        let x = values[0].f();
        let y = match op {
            "sqrt" => {
                if x < 0.0 {
                    return Err("MathDomainError");
                };
                if x == 0.0 {
                    0.0
                } else {
                    libm::sqrt(x)
                }
            }
            "sin" => libm::sin(x),
            "cos" => libm::cos(x),
            "exp" => libm::exp(x),
            "log" => math_log(&values[0])?,
            "power" => {
                let y = values[1].f();
                if x < 0.0 && y.fract() != 0.0 {
                    return Err("MathDomainError");
                };
                libm::pow(x, y)
            }
            _ => return Err("UnsupportedExpressionError"),
        };
        let out = N::Float(y);
        self.crossing(op, values, "approximated", Some(&out));
        Ok(out)
    }
    fn eval(&mut self, v: &J) -> Result<N, E> {
        match text(v, "type")? {
            "number" => {
                let s = text(v, "text")?;
                let integer = !s.contains(['.', 'e', 'E']);
                if integer || self.strict {
                    Ok(N::Exact(qparse(s)?, integer))
                } else {
                    let out = N::Float(s.parse().map_err(|_| "UnsupportedExpressionError")?);
                    self.boundaries.push(json!({"operation":"number literal","operands":[{"kind":"literal","text":s}],"operand_accuracy":"exact","action":"approximated","result":out.wire()}));
                    Ok(out)
                }
            }
            "variable" => self
                .bindings
                .get(text(v, "name")?)
                .cloned()
                .ok_or("MissingVariableError"),
            "unary" => {
                let n = self.eval(&v["operand"])?;
                Ok(if text(v, "operator")? == "-" {
                    n.neg()
                } else {
                    n
                })
            }
            "call" => {
                let n = self.eval(&v["argument"])?;
                let f = text(v, "name")?;
                if f == "abs" {
                    return Ok(if n.negative() { n.neg() } else { n });
                };
                if f == "sqrt" && n.exact() {
                    let q = n.q().unwrap();
                    if !q.is_negative() {
                        let a = q.numer().sqrt();
                        let b = q.denom().sqrt();
                        if &a * &a == *q.numer() && &b * &b == *q.denom() {
                            return Ok(N::Exact(Q::new(a, b), false));
                        }
                    }
                };
                if (!self.strict || !n.exact()) && f != "sqrt" {
                    let repeated = self.eval(&v["argument"])?;
                    self.limited(if f == "ln" { "log" } else { f }, &[repeated])
                } else {
                    self.limited(if f == "ln" { "log" } else { f }, &[n])
                }
            }
            "binary" => {
                let a = self.eval(&v["left"])?;
                let b = self.eval(&v["right"])?;
                let op = text(v, "operator")?;
                if op == "/" && b.zero() {
                    return Err("DivisionByZeroError");
                };
                if op == "^" {
                    if matches!(a, N::Decimal(..)) || matches!(b, N::Decimal(..)) {
                        let out = decimal_math::binary(op, &a, &b)?;
                        self.crossing("power", &[a, b], "approximated", Some(&out));
                        return Ok(out);
                    }
                    if a.exact() && b.exact() {
                        let base = a.q().unwrap();
                        let exp = b.q().unwrap();
                        if base.is_zero() && exp.is_negative() {
                            return Err("DivisionByZeroError");
                        };
                        if exp.denom().is_one() {
                            let integer =
                                matches!(a, N::Exact(_, true)) && matches!(b, N::Exact(_, true));
                            if base.is_one() {
                                return Ok(N::Exact(Q::one(), integer));
                            }
                            if base == Q::from_integer((-1).into()) {
                                return Ok(N::Exact(
                                    Q::from_integer(if exp.numer().is_odd() {
                                        (-1).into()
                                    } else {
                                        1.into()
                                    }),
                                    integer,
                                ));
                            }
                            if base.is_zero() {
                                return Ok(N::Exact(
                                    if exp.is_zero() { Q::one() } else { Q::zero() },
                                    integer,
                                ));
                            }

                            let n = exp.numer().to_i32().ok_or("UnsupportedExpressionError")?;
                            if n.unsigned_abs() > 100000 {
                                return Err("UnsupportedExpressionError");
                            };
                            return Ok(N::Exact(
                                base.pow(n),
                                matches!(a, N::Exact(_, true))
                                    && matches!(b, N::Exact(_, true))
                                    && n >= 0,
                            ));
                        };
                        if base.is_negative() {
                            return Err("MathDomainError");
                        };
                        if base.is_zero() || base.is_one() {
                            return Ok(N::Exact(base, false));
                        }
                    }
                    return self.limited("power", &[a, b]);
                };
                if a.exact() && b.exact() && (op != "/" || self.strict) {
                    let l = a.q().unwrap();
                    let r = b.q().unwrap();
                    let q = match op {
                        "+" => l + r,
                        "-" => l - r,
                        "*" => l * r,
                        "/" => l / r,
                        _ => return Err("InvalidRequest"),
                    };
                    return Ok(N::Exact(
                        q,
                        op != "/"
                            && matches!(a, N::Exact(_, true))
                            && matches!(b, N::Exact(_, true)),
                    ));
                };
                if matches!(a, N::Decimal(..)) || matches!(b, N::Decimal(..)) {
                    return decimal_math::binary(op, &a, &b);
                };
                let result = N::Float(match op {
                    "+" => a.f() + b.f(),
                    "-" => a.f() - b.f(),
                    "*" => a.f() * b.f(),
                    "/" => a.f() / b.f(),
                    _ => return Err("InvalidRequest"),
                });
                if op == "/" && !self.strict {
                    self.crossing("divide", &[a, b], "approximated", Some(&result))
                };
                Ok(result)
            }
            _ => Err("InvalidRequest"),
        }
    }
}
fn run(req: &J) -> Result<J, E> {
    keys(
        req,
        &["schema", "ast", "bindings", "mode", "guard_digits"],
        &["schema", "ast"],
    )?;
    if req["schema"] != SCHEMA {
        return Err("InvalidRequest");
    };
    let mode = req
        .get("mode")
        .map_or(Some("strict"), J::as_str)
        .ok_or("InvalidRequest")?;
    if mode != "strict" && mode != "approximate" {
        return Err("InvalidRequest");
    };
    let guard = match req.get("guard_digits") {
        None | Some(J::Null) => None,
        Some(x) => {
            let n = x.as_f64().ok_or("InvalidRequest")?;
            if n <= 0.0 || n > 1000.0 || n.fract() != 0.0 {
                return Err("InvalidRequest");
            };
            Some(n as u32)
        }
    };
    let mut raw_bindings = Vec::new();
    if let Some(m) = req.get("bindings") {
        for (k, v) in m.as_object().ok_or("InvalidRequest")? {
            let s = v.as_str().ok_or("InvalidRequest")?;
            if !name(k) || !decimal(s) {
                return Err("InvalidRequest");
            }
            raw_bindings.push((k.clone(), s.to_owned()));
        }
    }
    ast(&req["ast"], 0)?;
    let mut bindings = BTreeMap::new();
    for (key, value) in raw_bindings {
        let s = value.as_str();
        if !binding_decimal(s) {
            return Err("InvalidBindingError");
        }
        bindings.insert(
            key,
            if mode == "strict" {
                N::Exact(qparse(s)?, false)
            } else {
                N::Float(s.parse().map_err(|_| "InvalidBindingError")?)
            },
        );
    }
    let mut ev = Eval {
        strict: mode == "strict",
        guard,
        bindings,
        boundaries: vec![],
    };
    match ev.eval(&req["ast"]) {
        Ok(n) => {
            if matches!(n,N::Float(x) if !x.is_finite())
                || matches!(&n,N::Decimal(d) if !d.finite())
            {
                return Err("NonFiniteResultError");
            };
            Ok(json!({"value":n.wire(),"boundaries":ev.boundaries,"deferred":false}))
        }
        Err("Deferred") => Ok(
            json!({"value":null,"boundaries":ev.boundaries,"deferred":true,"unevaluated_ast":req["ast"]}),
        ),
        Err(e) => Err(e),
    }
}
pub fn evaluate_json(input: &str) -> String {
    // Serde RawValue remains syntax authority; preserve the original numeric
    //127-container/finite-number policy despite workspace feature unification.
    let parsed = plurimath_json::decode_with_limits(
        input.as_bytes(),
        127,
        usize::MAX,
        plurimath_json::NumberMode::Finite,
    )
    .map(|mut request| {
        request.sort_all_objects();
        request
    })
    .map_err(|_| "InvalidRequest");
    if let Ok(request) = &parsed {
        if request["schema"] == gum::SCHEMA {
            return serialize(gum::run(request));
        }
        if request["schema"] == QUANTITY_SCHEMA {
            return quantity_wire(request);
        }
    }
    let mut out =
        json!({"schema":SCHEMA,"engine":"plurimath.numeric/1","grammar":"plurimath.numeric-ast/1"});
    match parsed.and_then(|request| run(&request)) {
        Ok(result) => {
            out["ok"] = json!(true);
            out["result"] = result;
        }
        Err(error) => {
            out["ok"] = json!(false);
            out["error"] = json!({"code":error});
        }
    }
    serialize(out)
}

fn serialize(mut output: J) -> String {
    // AsciiUnits enables serde_json's preserve_order feature. Keep both
    // profiles byte-stable regardless of Cargo feature unification.
    output.sort_all_objects();
    plurimath_json::canonicalize_finite_numbers(&mut output);
    serde_json::to_string(&output).expect("JSON value serialization")
}

fn quantity_wire(_request: &J) -> String {
    let mut out = json!({
        "schema": QUANTITY_SCHEMA,
        "engine": "plurimath.quantity/1",
        "grammar": "plurimath.numeric-ast/1",
    });
    #[cfg(feature = "quantity")]
    let result = quantity::run(_request);
    #[cfg(not(feature = "quantity"))]
    let result: Result<J, J> = Err(json!({"code": "UnavailableProfileError"}));
    match result {
        Ok(value) => {
            out["ok"] = json!(true);
            out["result"] = value;
        }
        Err(error) => {
            out["ok"] = json!(false);
            out["error"] = error;
        }
    }
    serialize(out)
}

// ABI v1: borrowed UTF-8 bytes in, owned NUL-terminated UTF-8 JSON out.
// Exactly one result_free call releases a non-null returned pointer.
#[no_mangle]
pub extern "C" fn plurimath_numeric_abi_version() -> u32 {
    1
}
/// # Safety
/// Input must be null or point to at least `len` readable bytes for this call.
/// The caller must free each non-null return once with `plurimath_numeric_result_free`.
#[no_mangle]
pub unsafe extern "C" fn plurimath_numeric_evaluate(
    input: *const u8,
    len: usize,
) -> *mut std::ffi::c_char {
    let run = std::panic::catch_unwind(|| {
        let bytes = if input.is_null() || len == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(input, len)
        };
        let wire = evaluate_json(std::str::from_utf8(bytes).unwrap_or(""));
        std::ffi::CString::new(wire)
            .expect("JSON escapes NUL")
            .into_raw()
    });
    run.unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// A non-null pointer must be an unreleased return from `plurimath_numeric_evaluate`.
#[no_mangle]
pub unsafe extern "C" fn plurimath_numeric_result_free(result: *mut std::ffi::c_char) {
    if !result.is_null() {
        drop(std::ffi::CString::from_raw(result));
    }
}
#[no_mangle]
pub extern "C" fn plurimath_numeric_alloc(len: usize) -> *mut u8 {
    let bytes = vec![0u8; len].into_boxed_slice();
    Box::into_raw(bytes) as *mut u8
}
/// # Safety
/// A non-null pointer and length must match an unreleased `plurimath_numeric_alloc` allocation.
#[no_mangle]
pub unsafe extern "C" fn plurimath_numeric_dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
    }
}
