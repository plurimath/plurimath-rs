//! Dual and hyper-dual automatic differentiation (issue #17).
//!
//! `Dual` carries a value and one directional first derivative, so one pass
//! per input gives the exact-to-rounding gradient. `HyperDual` seeded with the
//! same direction in both nilpotent slots carries `f`, `f_i` and `f_ii`, which
//! is the diagonal Hessian entry the GUM H.1.7 second-order correction needs
//! for independent inputs. Both implement the crate's `Scalar` trait (the
//! numeric tower of #16), so any model written against `Scalar` runs on them.
//!
//! Results are class `limited`: machine-precision AD is not `exact`; the `Q`
//! evaluator stays the oracle.
use crate::scalar::Scalar;
use crate::E;
use serde_json::Value as J;
use std::collections::BTreeMap;

/// Value plus first derivative along one seeded direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dual {
    pub v: f64,
    pub d: f64,
}

/// Value, two first-order slots and their mixed term (`e1 e2`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperDual {
    pub v: f64,
    pub e1: f64,
    pub e2: f64,
    pub e12: f64,
}

/// Scalars that can evaluate an AST: a field algebra plus composition with a
/// smooth unary function given its value and first two derivatives at `self`.
trait Jet: Clone {
    fn constant(v: f64) -> Self;
    fn value(&self) -> f64;
    fn jadd(&self, o: &Self) -> Self;
    fn jsub(&self, o: &Self) -> Self;
    fn jmul(&self, o: &Self) -> Self;
    fn compose(&self, g: f64, g1: f64, g2: f64) -> Self;
}

impl Jet for Dual {
    fn constant(v: f64) -> Self {
        Dual { v, d: 0.0 }
    }
    fn value(&self) -> f64 {
        self.v
    }
    fn jadd(&self, o: &Self) -> Self {
        Dual {
            v: self.v + o.v,
            d: self.d + o.d,
        }
    }
    fn jsub(&self, o: &Self) -> Self {
        Dual {
            v: self.v - o.v,
            d: self.d - o.d,
        }
    }
    fn jmul(&self, o: &Self) -> Self {
        Dual {
            v: self.v * o.v,
            d: self.v * o.d + self.d * o.v,
        }
    }
    fn compose(&self, g: f64, g1: f64, _g2: f64) -> Self {
        Dual {
            v: g,
            d: g1 * self.d,
        }
    }
}

impl Jet for HyperDual {
    fn constant(v: f64) -> Self {
        HyperDual {
            v,
            e1: 0.0,
            e2: 0.0,
            e12: 0.0,
        }
    }
    fn value(&self) -> f64 {
        self.v
    }
    fn jadd(&self, o: &Self) -> Self {
        HyperDual {
            v: self.v + o.v,
            e1: self.e1 + o.e1,
            e2: self.e2 + o.e2,
            e12: self.e12 + o.e12,
        }
    }
    fn jsub(&self, o: &Self) -> Self {
        HyperDual {
            v: self.v - o.v,
            e1: self.e1 - o.e1,
            e2: self.e2 - o.e2,
            e12: self.e12 - o.e12,
        }
    }
    fn jmul(&self, o: &Self) -> Self {
        HyperDual {
            v: self.v * o.v,
            e1: self.v * o.e1 + self.e1 * o.v,
            e2: self.v * o.e2 + self.e2 * o.v,
            e12: self.v * o.e12 + self.e1 * o.e2 + self.e2 * o.e1 + self.e12 * o.v,
        }
    }
    fn compose(&self, g: f64, g1: f64, g2: f64) -> Self {
        HyperDual {
            v: g,
            e1: g1 * self.e1,
            e2: g1 * self.e2,
            e12: g1 * self.e12 + g2 * self.e1 * self.e2,
        }
    }
}

fn finite(x: f64) -> Result<f64, E> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err("NonFiniteResultError")
    }
}

fn recip<J_: Jet>(x: &J_) -> Result<J_, E> {
    let v = x.value();
    if v == 0.0 {
        return Err("DivisionByZeroError");
    }
    Ok(x.compose(1.0 / v, -1.0 / (v * v), 2.0 / (v * v * v)))
}

fn sqrt_j<J_: Jet>(x: &J_) -> Result<J_, E> {
    let v = x.value();
    if v <= 0.0 {
        return Err(if v < 0.0 {
            "NonFiniteResultError"
        } else {
            "DivisionByZeroError"
        });
    }
    let s = libm::sqrt(v);
    Ok(x.compose(s, 0.5 / s, -0.25 / (s * v)))
}

fn exp_j<J_: Jet>(x: &J_) -> Result<J_, E> {
    let e = finite(libm::exp(x.value()))?;
    Ok(x.compose(e, e, e))
}

fn ln_j<J_: Jet>(x: &J_) -> Result<J_, E> {
    let v = x.value();
    if v <= 0.0 {
        return Err("MathDomainError");
    }
    Ok(x.compose(libm::log(v), 1.0 / v, -1.0 / (v * v)))
}

macro_rules! scalar_impl {
    ($t:ty) => {
        impl Scalar for $t {
            type Wide = $t;
            fn s_zero() -> Self {
                <Self as Jet>::constant(0.0)
            }
            fn s_one() -> Self {
                <Self as Jet>::constant(1.0)
            }
            fn s_is_zero(&self) -> bool {
                self.value() == 0.0
            }
            fn s_is_negative(&self) -> bool {
                self.value() < 0.0
            }
            fn from_i32(n: i32) -> Self {
                <Self as Jet>::constant(f64::from(n))
            }
            fn from_q(q: &crate::Q) -> Self {
                <Self as Jet>::constant(<f64 as Scalar>::from_q(q))
            }
            fn bit_size(&self) -> u64 {
                0
            }
            fn to_i32_exact(&self) -> Option<i32> {
                <f64 as Scalar>::to_i32_exact(&self.v)
            }
            fn add(&self, rhs: &Self) -> Self {
                Jet::jadd(self, rhs)
            }
            fn sub(&self, rhs: &Self) -> Self {
                Jet::jsub(self, rhs)
            }
            fn mul(&self, rhs: &Self) -> Self {
                Jet::jmul(self, rhs)
            }
            fn div(&self, rhs: &Self) -> Option<Self> {
                recip(rhs).ok().map(|r| Jet::jmul(self, &r))
            }
            fn sqrt(&self, _digits: u32) -> Result<Self, E> {
                sqrt_j(self)
            }
            fn exp(&self, _digits: u32) -> Result<Self, E> {
                exp_j(self)
            }
            fn ln(&self, _digits: u32) -> Result<Self, E> {
                ln_j(self)
            }
            /// Rounds the value only; derivative slots are left unrounded.
            fn quantize(&self, digits: u32) -> Self {
                let mut out = *self;
                out.v = Scalar::quantize(&self.v, digits);
                out
            }
        }
    };
}
scalar_impl!(Dual);
scalar_impl!(HyperDual);

fn number(node: &J) -> Result<f64, E> {
    node["text"]
        .as_str()
        .and_then(|t| t.parse::<f64>().ok())
        .filter(|x| x.is_finite())
        .ok_or("InvalidModelError")
}

fn eval<T: Jet + Scalar<Wide = T>>(node: &J, vars: &BTreeMap<&str, T>) -> Result<T, E> {
    match node["type"].as_str() {
        Some("number") => Ok(T::constant(number(node)?)),
        Some("variable") => vars
            .get(node["name"].as_str().unwrap_or(""))
            .cloned()
            .ok_or("MissingVariableError"),
        Some("unary") => {
            let x = eval(&node["operand"], vars)?;
            Ok(if node["operator"] == "-" {
                T::constant(0.0).jsub(&x)
            } else {
                x
            })
        }
        Some("binary") => {
            let op = node["operator"].as_str().unwrap_or("");
            if op == "^" {
                let base = eval(&node["left"], vars)?;
                let e = &node["right"];
                if e["type"] == "number" {
                    let p = number(e)?;
                    if p.fract() == 0.0 && p.abs() <= 64.0 {
                        let mut out = T::constant(1.0);
                        for _ in 0..(p.abs() as u32) {
                            out = out.jmul(&base);
                        }
                        return if p < 0.0 { recip(&out) } else { Ok(out) };
                    }
                }
                let exponent = eval(e, vars)?;
                return exp_j(&exponent.jmul(&ln_j(&base)?));
            }
            let (l, r) = (eval(&node["left"], vars)?, eval(&node["right"], vars)?);
            match op {
                "+" => Ok(l.jadd(&r)),
                "-" => Ok(l.jsub(&r)),
                "*" => Ok(l.jmul(&r)),
                "/" => Ok(l.jmul(&recip(&r)?)),
                _ => Err("InvalidModelError"),
            }
        }
        Some("call") => {
            let x = eval(&node["argument"], vars)?;
            let v = x.value();
            match node["name"].as_str() {
                Some("sin") => Ok(x.compose(libm::sin(v), libm::cos(v), -libm::sin(v))),
                Some("cos") => Ok(x.compose(libm::cos(v), -libm::sin(v), -libm::cos(v))),
                Some("exp") => exp_j(&x),
                Some("ln") => ln_j(&x),
                Some("sqrt") => sqrt_j(&x),
                _ => Err("UnsupportedModel"),
            }
        }
        _ => Err("InvalidModelError"),
    }
}

/// One model input: name, estimate and standard uncertainty.
#[derive(Clone, Debug)]
pub struct Input<'a> {
    pub name: &'a str,
    pub estimate: f64,
    pub uncertainty: f64,
}

/// First- and second-order AD results. `class` is always `"limited"`.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub class: &'static str,
    pub value: f64,
    /// `∂f/∂xᵢ` per input, in input order.
    pub gradient: Vec<f64>,
    /// Full Hessian `∂²f/∂xᵢ∂xⱼ`.
    pub hessian: Vec<Vec<f64>>,
    /// `Σᵢ cᵢ² uᵢ²` with `cᵢ = ∂f/∂xᵢ` (GUM first-order variance, independent inputs).
    pub first_order_variance: f64,
    /// `½ Σᵢⱼ (∂²f/∂xᵢ∂xⱼ)² uᵢ² uⱼ²`: the squared-Hessian second-order term of the
    /// note to GUM 5.1.2 (H.1.7). The `Σ fᵢ fᵢⱼⱼ` third-derivative term is not included.
    pub second_order_term: f64,
}

fn names<'a>(inputs: &'a [Input]) -> Result<Vec<&'a str>, E> {
    let mut seen = BTreeMap::new();
    for (i, input) in inputs.iter().enumerate() {
        if seen.insert(input.name, i).is_some() {
            return Err("InvalidModelError");
        }
    }
    Ok(inputs.iter().map(|i| i.name).collect())
}

fn seeded<T: Clone>(
    inputs: &[Input],
    names: &[&str],
    make: impl Fn(usize, f64) -> T,
) -> BTreeMap<String, T> {
    names
        .iter()
        .zip(inputs)
        .enumerate()
        .map(|(i, (n, x))| (n.to_string(), make(i, x.estimate)))
        .collect()
}

fn borrow<T>(m: &BTreeMap<String, T>) -> BTreeMap<&str, T>
where
    T: Clone,
{
    m.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()
}

/// Gradient by one `Dual` pass per input.
pub fn sensitivities(ast: &J, inputs: &[Input]) -> Result<Vec<f64>, E> {
    let names = names(inputs)?;
    (0..inputs.len())
        .map(|k| {
            let vars = seeded(inputs, &names, |i, v| Dual {
                v,
                d: f64::from(u8::from(i == k)),
            });
            finite(eval(ast, &borrow(&vars))?.d)
        })
        .collect()
}

/// Full Hessian by `HyperDual` passes seeded in directions `i` and `j`
/// (`i == j` gives the diagonal `∂²f/∂xᵢ²`).
#[allow(clippy::needless_range_loop)] // symmetric (i, j) fill
pub fn hessian(ast: &J, inputs: &[Input]) -> Result<Vec<Vec<f64>>, E> {
    let names = names(inputs)?;
    let n = inputs.len();
    let mut h = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in i..n {
            let vars = seeded(inputs, &names, |k, v| HyperDual {
                v,
                e1: f64::from(u8::from(k == i)),
                e2: f64::from(u8::from(k == j)),
                e12: 0.0,
            });
            let value = finite(eval(ast, &borrow(&vars))?.e12)?;
            h[i][j] = value;
            h[j][i] = value;
        }
    }
    Ok(h)
}

/// Hessian diagonal (`∂²f/∂xᵢ²`).
pub fn hessian_diagonal(ast: &J, inputs: &[Input]) -> Result<Vec<f64>, E> {
    let h = hessian(ast, inputs)?;
    Ok((0..h.len()).map(|i| h[i][i]).collect())
}

/// Value, gradient, Hessian and the H.1.7 second-order term.
pub fn analyze(ast: &J, inputs: &[Input]) -> Result<Report, E> {
    let gradient = sensitivities(ast, inputs)?;
    let hessian = hessian(ast, inputs)?;
    let names = names(inputs)?;
    let vars = seeded(inputs, &names, |_, v| Dual { v, d: 0.0 });
    let value = finite(eval(ast, &borrow(&vars))?.v)?;
    let u2 = |i: usize| inputs[i].uncertainty * inputs[i].uncertainty;
    let n = inputs.len();
    let first_order_variance = (0..n).map(|i| gradient[i] * gradient[i] * u2(i)).sum();
    let second_order_term = 0.5
        * (0..n)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| hessian[i][j] * hessian[i][j] * u2(i) * u2(j))
            .sum::<f64>();
    Ok(Report {
        class: "limited",
        value,
        gradient,
        hessian,
        first_order_variance,
        second_order_term,
    })
}
