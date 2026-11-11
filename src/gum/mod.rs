use super::{decimal, guard_sqrt, qparse, roundq, BigInt, J, N, Q};
use num_traits::{Signed, Zero};
use serde_json::json;

mod calibration;
mod conformity;
mod coverage;
mod implicit;
mod monte_carlo;
mod propagation;
mod sampling_model;
mod statistics;
mod taylor;

pub(crate) const SCHEMA: &str = "plurimath.evaluate.gum/1";
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct Error {
    code: &'static str,
    message: &'static str,
}

fn invalid(message: &'static str) -> Error {
    Error {
        code: "InvalidInput",
        message,
    }
}

fn unsupported(message: &'static str) -> Error {
    Error {
        code: "UnsupportedModel",
        message,
    }
}

fn fields(value: &J, allowed: &[&str]) -> Result<()> {
    if value
        .as_object()
        .is_none_or(|object| object.keys().any(|key| !allowed.contains(&key.as_str())))
    {
        return Err(invalid("unexpected request fields"));
    }
    Ok(())
}

fn exact(value: &J) -> Result<Q> {
    if let Some(integer) = value.as_i64() {
        return Ok(Q::from_integer(integer.into()));
    }
    if let Some(integer) = value.as_u64() {
        return Ok(Q::from_integer(integer.into()));
    }
    if let Some(text) = value.as_str() {
        return exact_text(text);
    }
    if value.as_object().is_some_and(|object| object.len() == 1) {
        if let Some(text) = value["decimal"].as_str() {
            return exact_text(text);
        }
        if let Some(parts) = value["rational"].as_array().filter(|pair| pair.len() == 2) {
            return ratio(
                parts[0].as_str().unwrap_or(""),
                parts[1].as_str().unwrap_or(""),
            );
        }
    }
    Err(invalid("invalid exact numeric input"))
}

fn exact_text(text: &str) -> Result<Q> {
    if text.len() > 4096 {
        return Err(invalid("invalid exact numeric input"));
    }
    if let Some((n, d)) = text.split_once('/') {
        return ratio(n, d);
    }
    if !decimal(text)
        || text.split_once(['e', 'E']).is_some_and(|(_, exponent)| {
            exponent
                .parse::<i32>()
                .map_or(true, |n| n.unsigned_abs() > 4096)
        })
    {
        return Err(invalid("invalid exact numeric input"));
    }
    qparse(text).map_err(|_| invalid("invalid exact numeric input"))
}

fn ratio(n: &str, d: &str) -> Result<Q> {
    let integer = |s: &str| {
        let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
        if digits.is_empty()
            || digits.len() > 4096
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid("invalid exact numeric input"));
        }
        s.parse::<BigInt>()
            .map_err(|_| invalid("invalid exact numeric input"))
    };
    let denominator = integer(d)?;
    if denominator.is_zero() {
        return Err(invalid("invalid exact numeric input"));
    }
    Ok(Q::new(integer(n)?, denominator))
}

fn tagged(value: &Q) -> J {
    json!({"rational":[value.numer().to_string(),value.denom().to_string()]})
}

fn symbol(value: &str) -> J {
    json!({"symbol":value})
}

fn symbol_name(value: &J) -> Option<&str> {
    value.as_str().or_else(|| {
        if value.as_object()?.len() == 1 {
            value["symbol"].as_str()
        } else {
            None
        }
    })
}

fn vector(value: &J, message: &'static str) -> Result<Vec<Q>> {
    value
        .as_array()
        .filter(|v| v.len() <= 4096)
        .ok_or_else(|| invalid(message))?
        .iter()
        .map(exact)
        .collect()
}

fn matrix(value: &J, message: &'static str, minimum_rows: usize) -> Result<Vec<Vec<Q>>> {
    let rows = value
        .as_array()
        .filter(|rows| rows.len() >= minimum_rows && rows.len() <= 4096)
        .ok_or_else(|| invalid(message))?;
    let width = rows.first().and_then(J::as_array).map_or(0, Vec::len);
    if width == 0
        || width > 64
        || rows
            .iter()
            .any(|row| row.as_array().is_none_or(|values| values.len() != width))
    {
        return Err(invalid(message));
    }
    rows.iter().map(|row| vector(row, message)).collect()
}

fn number(n: usize) -> BigInt {
    n.into()
}

fn guard(value: &J, max: u32, message: &'static str) -> Result<u32> {
    value
        .as_u64()
        .filter(|&n| n > 0 && n <= u64::from(max))
        .map(|n| n as u32)
        .ok_or_else(|| invalid(message))
}

fn fmt_decimal(value: N) -> String {
    let N::Decimal(value) = value else {
        unreachable!("rounded decimal")
    };
    if value.c.is_zero() {
        return "0".to_owned();
    }
    let mut digits = value.c.abs().to_string();
    if value.s >= 0 {
        digits.push_str(&"0".repeat(value.s as usize));
    } else {
        let places = value.s.unsigned_abs() as usize;
        if digits.len() <= places {
            digits = format!("{}{}", "0".repeat(places + 1 - digits.len()), digits);
        }
        digits.insert(digits.len() - places, '.');
        digits = digits
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    if value.c.is_negative() {
        digits.insert(0, '-');
    }
    digits
}

fn root_decimal(value: &Q, digits: u32) -> Result<String> {
    let quotient = roundq(value, digits).q().expect("finite rounded rational");
    guard_sqrt(&quotient, digits)
        .map(fmt_decimal)
        .map_err(|_| invalid("negative variance"))
}

fn decimal_payload(mut text: String) -> J {
    if !text.contains('.') {
        text.push_str(".0");
    }
    json!({"decimal":text})
}

fn engine(method: &str) -> J {
    json!({"name":"plurimath-rust","version":env!("CARGO_PKG_VERSION"),"method":method})
}

fn dispatch(request: &J) -> Result<J> {
    fields(request, &["schema", "method", "parameters"])?;
    let method = request["method"]
        .as_str()
        .ok_or_else(|| invalid("method must be a string"))?;
    let params = &request["parameters"];
    match method {
        "type_a" => statistics::type_a_request(params),
        "type_a_covariance" => statistics::covariance(params),
        "series" => statistics::series(params),
        "grouped_type_a" => statistics::grouped(params),
        "component" => propagation::component(params),
        "coverage" => coverage::evaluate(params),
        "conformity" => conformity::evaluate(params),
        "linear_calibration" => calibration::fit(params),
        "predict_calibration" => calibration::predict(params),
        "propagate_implicit" => implicit::propagate(params),
        "correlation_matrix" => propagation::correlations(params),
        "monte_carlo" => monte_carlo::evaluate(params),
        "propagate" => propagation::independent(params),
        "analyze" => taylor::analyze(params),
        "propagate_covariance" => propagation::correlated(params),
        "propagate_vector" => propagation::transform(params),
        _ => Err(unsupported("unsupported GUM method")),
    }
}

pub(crate) fn run(request: &J) -> J {
    let mut output = json!({"schema":SCHEMA,"engine":"plurimath.gum/1"});
    match dispatch(request) {
        Ok(value) => {
            output["ok"] = json!(true);
            output["result"] = value;
        }
        Err(error) => {
            output["ok"] = json!(false);
            output["error"] = json!({"code":error.code,"message":error.message});
        }
    }
    output
}
