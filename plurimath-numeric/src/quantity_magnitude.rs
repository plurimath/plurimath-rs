//! Preserve caller numeric classes separately from exact unit conversion.
use super::{binding_decimal, decimal, decimal_math::D, keys, qparse, Eval, J, N, Q};
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};
use std::collections::BTreeMap;

pub(super) fn rational(value: &N) -> Result<Q, &'static str> {
    value
        .q()
        .or_else(|| match value {
            N::Float(f) if f.is_finite() => Q::from_float(*f),
            _ => None,
        })
        .ok_or("NonFiniteResultError")
}

pub(super) fn binding(value: &J) -> Result<N, &'static str> {
    if let Some(s) = value.as_str() {
        return if binding_decimal(s) {
            Ok(N::Exact(qparse(s)?, false))
        } else {
            Err("InvalidBindingError")
        };
    }
    if value.is_number() {
        return if value.as_i64().is_some() || value.as_u64().is_some() {
            Ok(N::Exact(qparse(&value.to_string())?, true))
        } else {
            Err("InvalidBindingError")
        };
    }
    if let Some(s) = value.get("decimal").and_then(J::as_str) {
        keys(value, &["decimal"], &["decimal"])?;
        if !decimal(s) {
            return Err("InvalidBindingError");
        }
        // The existing numeric parser supplies the same exponent/resource guard.
        qparse(s)?;
        let mut parts = s.split(['e', 'E']);
        let mantissa = parts.next().unwrap();
        let exponent = parts
            .next()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|_| "InvalidBindingError")?
            .unwrap_or(0);
        let after = mantissa.split('.').nth(1).map_or(0, str::len) as i64;
        let coefficient: BigInt = mantissa
            .replace('.', "")
            .parse()
            .map_err(|_| "InvalidBindingError")?;
        let negative_zero = coefficient.is_zero() && s.starts_with('-');
        return Ok(N::Decimal(D::new(
            coefficient,
            exponent - after,
            negative_zero,
        )));
    }
    if let Some(parts) = value.get("rational").and_then(J::as_array) {
        keys(value, &["rational"], &["rational"])?;
        if parts.len() != 2 {
            return Err("InvalidBindingError");
        }
        let integer = |v: &J| -> Result<BigInt, &'static str> {
            v.as_str()
                .ok_or("InvalidBindingError")?
                .parse()
                .map_err(|_| "InvalidBindingError")
        };
        let numerator = integer(&parts[0])?;
        let denominator = integer(&parts[1])?;
        if denominator.is_zero() {
            return Err("InvalidBindingError");
        }
        return Ok(N::Exact(Q::new(numerator, denominator), false));
    }
    if let Some(bits) = value.get("binary64").and_then(J::as_str) {
        keys(value, &["binary64"], &["binary64"])?;
        if bits.len() != 16 || !bits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("InvalidBindingError");
        }
        let n = f64::from_bits(u64::from_str_radix(bits, 16).map_err(|_| "InvalidBindingError")?);
        return if n.is_finite() {
            Ok(N::Float(n))
        } else {
            Err("InvalidBindingError")
        };
    }
    Err("InvalidBindingError")
}

pub(super) fn binary(op: &str, left: N, right: N) -> Result<N, &'static str> {
    let mut evaluator = Eval {
        strict: true,
        guard: None,
        bindings: BTreeMap::from([("left".into(), left), ("right".into(), right)]),
        boundaries: Vec::new(),
    };
    evaluator.eval(&serde_json::json!({"type":"binary","operator":op,
        "left":{"type":"variable","name":"left"},
        "right":{"type":"variable","name":"right"}}))
}

pub(super) fn power(mut base: N, exponent: &Q) -> Result<N, &'static str> {
    let mut n = exponent
        .to_integer()
        .to_i32()
        .ok_or("UnsupportedExpressionError")?;
    let negative = n < 0;
    n = n.checked_abs().ok_or("UnsupportedExpressionError")?;
    let mut result = N::Exact(Q::one(), true);
    while n != 0 {
        if n % 2 == 1 {
            result = binary("*", result, base.clone())?;
        }
        n /= 2;
        if n != 0 {
            base = binary("*", base.clone(), base)?;
        }
    }
    if negative {
        binary("/", N::Exact(Q::one(), true), result)
    } else {
        Ok(result)
    }
}
