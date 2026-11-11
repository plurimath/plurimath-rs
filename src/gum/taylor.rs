use super::*;
use num_traits::{One, ToPrimitive};
use std::collections::BTreeMap;

/// Degree-three expansion about the supplied estimates. Monomials are sorted
/// variable indices, so each mixed partial has one exact coefficient.
#[derive(Clone, Default)]
struct Taylor(BTreeMap<Vec<usize>, Q>);

impl Taylor {
    fn constant(value: Q) -> Self {
        let mut terms = Self::default();
        terms.insert(Vec::new(), value);
        terms
    }

    fn variable(value: Q, index: usize) -> Self {
        let mut terms = Self::constant(value);
        terms.insert(vec![index], Q::one());
        terms
    }

    fn insert(&mut self, powers: Vec<usize>, coefficient: Q) {
        if coefficient.is_zero() {
            return;
        }
        let value = self.0.entry(powers.clone()).or_default();
        *value += coefficient;
        if value.is_zero() {
            self.0.remove(&powers);
        }
    }

    fn derivative(&self, indices: &[usize]) -> Q {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        let factor = match sorted.as_slice() {
            [a, b, c] if a == b && b == c => 6,
            [a, b, c] if a == b || b == c => 2,
            [a, b] if a == b => 2,
            _ => 1,
        };
        self.0
            .get(&sorted)
            .map_or_else(Q::zero, |value| value * number(factor))
    }

    fn add(mut self, rhs: Self) -> Self {
        for (key, value) in rhs.0 {
            self.insert(key, value);
        }
        self
    }

    fn scale(mut self, value: &Q) -> Self {
        if value.is_zero() {
            return Self::default();
        }
        self.0
            .values_mut()
            .for_each(|coefficient| *coefficient *= value);
        self
    }

    fn multiply<const TRUNCATE: bool>(&self, rhs: &Self) -> Result<Self> {
        let mut out = Self::default();
        for (a, x) in &self.0 {
            for (b, y) in &rhs.0 {
                let degree = a.len() + b.len();
                if TRUNCATE && degree > 3 {
                    continue;
                }
                if !TRUNCATE && degree > 32 {
                    return Err(unsupported("sampling polynomial degree exceeds32"));
                }
                let mut powers = a.clone();
                powers.extend(b);
                powers.sort_unstable();
                if x.numer().bits().max(x.denom().bits()) + y.numer().bits().max(y.denom().bits())
                    > 1_000_000
                {
                    return Err(unsupported(
                        "analytic derivative exceeds the bounded rational domain",
                    ));
                }
                out.insert(powers, x * y);
                if !TRUNCATE && out.0.len() > 128 {
                    return Err(unsupported("polynomial exceeds the bounded term domain"));
                }
            }
        }
        Ok(out)
    }

    fn reciprocal<const TRUNCATE: bool>(mut self) -> Result<Self> {
        let constant = self.0.remove(&Vec::new()).ok_or(Error {
            code: "DivisionByZeroError",
            message: "analytic denominator is zero at the estimate",
        })?;
        let reciprocal = constant.recip();
        if !TRUNCATE {
            if !self.0.is_empty() {
                return Err(unsupported(
                    "rational sampling models are outside this profile",
                ));
            }
            return Ok(Self::constant(reciprocal));
        }
        let delta = self.scale(&-&reciprocal);
        let one = Self::constant(Q::one());
        let mut sum = one.clone();
        let mut power = one;
        for _ in 0..3 {
            power = power.multiply::<TRUNCATE>(&delta)?;
            sum = sum.add(power.clone());
        }
        Ok(sum.scale(&reciprocal))
    }

    fn power<const TRUNCATE: bool>(mut self, exponent: Self) -> Result<Self> {
        if exponent.0.keys().any(|powers| !powers.is_empty()) {
            return Err(unsupported("variable exponents are unsupported"));
        }
        let exponent = exponent.derivative(&[]);
        if !exponent.is_integer() {
            return Err(unsupported(
                "analytic differentiation requires an integer exponent",
            ));
        }
        let exponent = exponent
            .to_integer()
            .to_i32()
            .filter(|n| n.unsigned_abs() <= 4096)
            .ok_or_else(|| unsupported("analytic exponent exceeds the bounded domain"))?;
        let mut n = exponent.unsigned_abs();
        let mut out = Self::constant(Q::one());
        while n > 0 {
            if n & 1 != 0 {
                out = out.multiply::<TRUNCATE>(&self)?;
            }
            n >>= 1;
            if n != 0 {
                self = self.multiply::<TRUNCATE>(&self)?;
            }
        }
        if exponent < 0 {
            out.reciprocal::<TRUNCATE>()
        } else {
            Ok(out)
        }
    }
}

fn walk<const TRUNCATE: bool>(node: &J, bindings: &BTreeMap<&str, Taylor>) -> Result<Taylor> {
    let value = match node["type"].as_str() {
        Some("number") => Ok(Taylor::constant(exact(&node["text"])?)),
        Some("variable") => bindings
            .get(node["name"].as_str().unwrap_or(""))
            .cloned()
            .ok_or(Error {
                code: "MissingVariableError",
                message: "missing analytic variable binding",
            }),
        Some("unary") => {
            let value = walk::<TRUNCATE>(&node["operand"], bindings)?;
            Ok(if node["operator"] == "-" {
                value.scale(&-Q::one())
            } else {
                value
            })
        }
        Some("binary") => {
            let left = walk::<TRUNCATE>(&node["left"], bindings)?;
            let right = walk::<TRUNCATE>(&node["right"], bindings)?;
            match node["operator"].as_str() {
                Some("+") => Ok(left.add(right)),
                Some("-") => Ok(left.add(right.scale(&-Q::one()))),
                Some("*") => left.multiply::<TRUNCATE>(&right),
                Some("/") => left.multiply::<TRUNCATE>(&right.reciprocal::<TRUNCATE>()?),
                Some("^") => left.power::<TRUNCATE>(right),
                _ => Err(invalid("invalid analytic AST")),
            }
        }
        Some("call") => Err(unsupported("analytic derivatives unavailable for calls")),
        _ => Err(invalid("invalid analytic AST")),
    }?;
    if !TRUNCATE && value.0.len() > 128 {
        return Err(unsupported("polynomial exceeds the bounded term domain"));
    }
    Ok(value)
}

pub(super) struct Polynomial {
    pub(super) offset: Q,
    pub(super) degrees: Vec<usize>,
    terms: Vec<(f64, Vec<usize>)>,
}

impl Polynomial {
    pub(super) fn centered_value(&self, deviations: &[f64]) -> f64 {
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (coefficient, indices) in &self.terms {
            let mut term = *coefficient;
            for &index in indices {
                term *= deviations[index];
            }
            let adjusted = term - correction;
            let total = sum + adjusted;
            correction = (total - sum) - adjusted;
            sum = total;
        }
        sum
    }
}

pub(super) fn sampling_polynomial(
    ast: &J,
    variables: &[&str],
    centers: Vec<Q>,
) -> Result<Polynomial> {
    crate::ast(ast, 0).map_err(|_| invalid("invalid sampling AST"))?;
    let bindings = variables
        .iter()
        .copied()
        .zip(centers)
        .enumerate()
        .map(|(i, (name, estimate))| (name, Taylor::variable(estimate, i)))
        .collect();
    let jet = walk::<false>(ast, &bindings)?;
    let offset = jet.derivative(&[]);
    let mut degrees = vec![0; variables.len()];
    let mut terms = BTreeMap::new();
    for (indices, value) in jet.0 {
        if indices.is_empty() {
            continue;
        }
        let coefficient = value
            .to_f64()
            .filter(|v| v.is_finite() && *v != 0.0)
            .ok_or_else(|| unsupported("centered model coefficient outside binary64 range"))?;
        let mut powers = vec![0; variables.len()];
        for &index in &indices {
            powers[index] += 1;
            degrees[index] = degrees[index].max(powers[index]);
        }
        terms.insert(powers, (coefficient, indices));
    }
    if terms.is_empty() {
        return Err(unsupported(
            "constant models need exact analysis rather than adaptive uncertainty tolerance",
        ));
    }
    Ok(Polynomial {
        offset,
        degrees,
        terms: terms.into_values().collect(),
    })
}

pub(super) fn analyze(params: &J) -> Result<J> {
    fields(
        params,
        &["ast", "inputs", "independent", "order", "guard_digits"],
    )?;
    if params["independent"] != true {
        return Err(unsupported("joint dependence is unsupported"));
    }
    crate::ast(&params["ast"], 0).map_err(|_| invalid("invalid analytic AST"))?;
    let inputs = params["inputs"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 16)
        .ok_or_else(|| invalid("analytic inputs must be a nonempty ordered list"))?;
    let mut bindings = BTreeMap::new();
    let mut variables = Vec::new();
    let mut components = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        fields(input, &["name", "estimate", "component"])?;
        let name = input["name"]
            .as_str()
            .filter(|name| crate::name(name))
            .ok_or_else(|| invalid("variable names must be distinct strings"))?;
        if bindings
            .insert(name, Taylor::variable(exact(&input["estimate"])?, i))
            .is_some()
        {
            return Err(invalid("variable names must be distinct strings"));
        }
        variables.push(name);
        components.push(input["component"].clone());
    }
    let jet = walk::<true>(&params["ast"], &bindings)?;
    let size = inputs.len();
    let gradient: Vec<_> = (0..size).map(|i| tagged(&jet.derivative(&[i]))).collect();
    let hessian: Vec<_> = (0..size)
        .map(|i| {
            (0..size)
                .map(|j| tagged(&jet.derivative(&[i, j])))
                .collect::<Vec<_>>()
        })
        .collect();
    let third: Vec<_> = (0..size)
        .map(|i| {
            (0..size)
                .map(|j| {
                    (0..size)
                        .map(|k| tagged(&jet.derivative(&[i, j, k])))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut out = propagation::independent(&json!({
        "estimate":tagged(&jet.derivative(&[])),"components":components,"gradient":gradient,
        "independent":true,"order":params.get("order").cloned().unwrap_or(json!(1)),
        "guard_digits":params["guard_digits"],"hessian":hessian,"third":third,"derivatives_complete":true,
    }))?;
    out["variables"] = json!(variables);
    out["gradient"] = json!(gradient);
    out["hessian"] = json!(hessian);
    out["third"] = json!(third);
    Ok(out)
}
