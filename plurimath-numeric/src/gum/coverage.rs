use super::*;
use num_traits::ToPrimitive;

pub(super) fn evaluate(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "standard_uncertainty",
            "probability",
            "degrees_of_freedom",
            "guard_digits",
        ],
    )?;
    let digits = guard(
        &params["guard_digits"],
        8,
        "coverage factors support 1..8 declared digits",
    )?;
    let probability = exact(&params["probability"])?;
    if probability < Q::new(number(1), number(2)) || probability > Q::new(number(999), number(1000))
    {
        return Err(unsupported(
            "verified coverage probability domain is 0.5..0.999",
        ));
    }
    let uncertainty = exact(&params["standard_uncertainty"])?;
    if uncertainty.is_negative() {
        return Err(invalid("negative standard uncertainty"));
    }
    let binary = uncertainty
        .to_f64()
        .filter(|n| n.is_finite() && (uncertainty.is_zero() || *n != 0.0))
        .ok_or_else(|| unsupported("standard uncertainty outside binary64 range"))?;
    let degrees = if params["degrees_of_freedom"].is_null() {
        None
    } else {
        Some(exact(&params["degrees_of_freedom"])?)
    };
    if degrees
        .as_ref()
        .is_some_and(|n| n < &Q::from_integer(number(1)) || n > &Q::from_integer(number(10_000)))
    {
        return Err(unsupported(
            "verified Student-t degrees of freedom domain is 1..10000",
        ));
    }
    let target = ((Q::from_integer(number(1)) + &probability) / number(2))
        .to_f64()
        .expect("bounded probability");
    let nu = degrees
        .as_ref()
        .map(|value| value.to_f64().expect("bounded degrees of freedom"));
    let factor = quantile(target, nu)?;
    let output = |value| {
        Q::from_float(value)
            .map(|q| fmt_decimal(roundq(&q, digits)))
            .ok_or_else(|| numerical("non-finite statistical result"))
    };
    let fraction = |q: &Q| format!("{}/{}", q.numer(), q.denom());
    let mut stamp = engine("normalized-density-coverage-v1");
    stamp["numeric_backend"] = json!("binary64");
    Ok(json!({
        "coverage_factor":output(factor)?,"expanded_uncertainty":output(binary*factor)?,
        "probability":fraction(&probability),"degrees_of_freedom":degrees.as_ref().map(fraction),
        "guard_digits":digits,"precision_convention":symbol("significant_digits"),
        "distribution":symbol(if degrees.is_none() {"normal"} else {"student_t"}),"engine":stamp,
    }))
}

fn numerical(message: &'static str) -> Error {
    Error {
        code: "NumericalFailure",
        message,
    }
}

pub(super) fn normal_cdf(value: f64) -> f64 {
    0.5 * libm::erfc(-value / std::f64::consts::SQRT_2)
}

fn quantile(probability: f64, nu: Option<f64>) -> Result<f64> {
    let cdf = |x| match nu {
        Some(nu) => student_cdf(x, nu),
        None => Ok(normal_cdf(x)),
    };
    let mut lower = 0.0;
    let mut upper = 1.0;
    while cdf(upper)? < probability && upper < 1_000_000.0 {
        upper *= 2.0;
    }
    if cdf(upper)? < probability {
        return Err(numerical("quantile could not be bracketed"));
    }
    for _ in 0..64 {
        let midpoint = (lower + upper) / 2.0;
        if cdf(midpoint)? < probability {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    Ok((lower + upper) / 2.0)
}

fn student_cdf(value: f64, nu: f64) -> Result<f64> {
    if value == 0.0 {
        return Ok(0.5);
    }
    let log_constant = libm::lgamma((nu + 1.0) / 2.0)
        - libm::lgamma(nu / 2.0)
        - 0.5 * libm::log(nu * std::f64::consts::PI);
    let density = |x: f64| {
        let u = x * x / nu;
        let one_plus = 1.0 + u;
        let log = if one_plus == 1.0 {
            u
        } else {
            libm::log(one_plus) * (u / (one_plus - 1.0))
        };
        libm::exp(log_constant - (nu + 1.0) / 2.0 * log)
    };
    let values = [density(0.0), density(value / 2.0), density(value)];
    let whole = value * (values[0] + 4.0 * values[1] + values[2]) / 6.0;
    let result = 0.5 + integrate(&density, [0.0, value], values, whole, 1e-13, 24)?;
    if !(0.0..=1.0 + 1e-11).contains(&result) {
        return Err(numerical("Student-t CDF outside numerical range"));
    }
    Ok(result.min(1.0))
}

fn integrate(
    f: &impl Fn(f64) -> f64,
    [left, right]: [f64; 2],
    [a, b, c]: [f64; 3],
    whole: f64,
    tolerance: f64,
    depth: u32,
) -> Result<f64> {
    let midpoint = (left + right) / 2.0;
    let d = f((left + midpoint) / 2.0);
    let e = f((midpoint + right) / 2.0);
    let l = (midpoint - left) * (a + 4.0 * d + b) / 6.0;
    let r = (right - midpoint) * (b + 4.0 * e + c) / 6.0;
    let error = l + r - whole;
    if error.abs() <= 15.0 * tolerance {
        return Ok(l + r + error / 15.0);
    }
    if depth == 0 {
        return Err(numerical("Student-t CDF quadrature did not converge"));
    }
    Ok(integrate(
        f,
        [left, midpoint],
        [a, d, b],
        l,
        tolerance / 2.0,
        depth - 1,
    )? + integrate(
        f,
        [midpoint, right],
        [b, e, c],
        r,
        tolerance / 2.0,
        depth - 1,
    )?)
}
