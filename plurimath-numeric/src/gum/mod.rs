use super::{decimal, qparse, roundq, scalar::Scalar, BigInt, J, N, Q};
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
mod validation;

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
    let quotient = Scalar::quantize(value, digits)
        .q()
        .expect("finite rounded rational");
    Scalar::sqrt(&quotient, digits)
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

/// Machine-precision AD sensitivities and second-order term; typed `limited`.
fn analyze_ad(params: &J) -> Result<J> {
    fields(params, &["ast", "inputs"])?;
    let items = params["inputs"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 16)
        .ok_or_else(|| invalid("analytic inputs must be a nonempty ordered list"))?;
    let mut inputs = Vec::new();
    for item in items {
        fields(item, &["name", "estimate", "uncertainty"])?;
        let number = |key: &str| {
            item[key]
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| invalid("inputs need numeric estimate and uncertainty"))
        };
        inputs.push(crate::ad::Input {
            name: item["name"]
                .as_str()
                .filter(|name| crate::name(name))
                .ok_or_else(|| invalid("variable names must be distinct strings"))?,
            estimate: number("estimate")?,
            uncertainty: number("uncertainty")?,
        });
    }
    let report = crate::ad::analyze(&params["ast"], &inputs).map_err(|code| match code {
        "UnsupportedModel" => unsupported("AD does not support this call"),
        _ => invalid("AD evaluation failed"),
    })?;
    Ok(json!({
        "class":report.class,"value":report.value,"gradient":report.gradient,
        "hessian":report.hessian,"first_order_variance":report.first_order_variance,
        "second_order_term":report.second_order_term,
    }))
}

/// Rigorous interval LPU-adequacy verdict (issue #18). The result carries the
/// verdict and the subdivision budget actually used and allowed.
fn lpu_adequacy(params: &J) -> Result<J> {
    fields(
        params,
        &["ast", "inputs", "coverage_factor", "threshold", "budget"],
    )?;
    let items = params["inputs"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 16)
        .ok_or_else(|| invalid("adequacy inputs must be a nonempty ordered list"))?;
    let (mut names, mut estimates, mut uncertainties) = (Vec::new(), Vec::new(), Vec::new());
    for item in items {
        fields(item, &["name", "estimate", "uncertainty"])?;
        let number = |key: &str| {
            item[key]
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| invalid("inputs need numeric estimate and uncertainty"))
        };
        let name = item["name"]
            .as_str()
            .filter(|name| crate::name(name))
            .filter(|name| !names.contains(name))
            .ok_or_else(|| invalid("variable names must be distinct strings"))?;
        names.push(name);
        estimates.push(number("estimate")?);
        uncertainties.push(number("uncertainty")?);
    }
    let positive = |key: &str| {
        params[key]
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| invalid("coverage_factor and threshold must be positive numbers"))
    };
    let budget = params["budget"]
        .as_u64()
        .filter(|b| (1..=100_000).contains(b))
        .ok_or_else(|| invalid("budget must be an integer from 1 to 100000"))?;
    let adequacy = crate::interval::lpu_adequacy_ast(
        &params["ast"],
        &names,
        &estimates,
        &uncertainties,
        positive("coverage_factor")?,
        positive("threshold")?,
        budget as usize,
    )
    .map_err(|code| match code {
        "UnsupportedModel" => unsupported("interval adequacy does not support this call"),
        "DegenerateUncertaintyError" => invalid("combined uncertainty is not provably positive"),
        _ => invalid("interval adequacy evaluation failed"),
    })?;
    let mut out = adequacy.to_json();
    out["engine"] = engine("interval-LPU-adequacy-v1");
    Ok(out)
}

fn dispatch(request: &J) -> Result<J> {
    fields(request, &["schema", "method", "parameters"])?;
    let method = request["method"]
        .as_str()
        .ok_or_else(|| invalid("method must be a string"))?;
    let (stripped, want_provenance) = take_provenance(&request["parameters"])?;
    let params = &stripped;
    let mut value = run_method(method, params)?;
    attach_provenance(&mut value, want_provenance);
    Ok(value)
}

/// Split the optional `provenance` flag off the method parameters so the
/// per-method field allowlists (and the frozen corpus) never see it.
fn take_provenance(params: &J) -> Result<(J, bool)> {
    let Some(object) = params.as_object() else {
        return Ok((params.clone(), false));
    };
    let Some(flag) = object.get("provenance") else {
        return Ok((params.clone(), false));
    };
    let flag = flag
        .as_bool()
        .ok_or_else(|| invalid("provenance must be a boolean"))?;
    let mut rest = object.clone();
    rest.remove("provenance");
    Ok((J::Object(rest), flag))
}

/// Add `engine.provenance` (additive, schema stays `/1`). Always for
/// gum-par; for every other engine only when requested, so frozen bytes
/// never move. Accuracy classes use the PM 1004 vocabulary. Results that are
/// not JSON objects (`correlation_matrix` returns a bare matrix) have no
/// place for an `engine` member and are left unchanged.
fn attach_provenance(result: &mut J, requested: bool) {
    let par = result.pointer("/engine/id").and_then(J::as_str) == Some("gum-par");
    if !(par || requested) {
        return;
    }
    let sampled = result.pointer("/engine/generator").is_some();
    let derived = if sampled {
        J::Null
    } else {
        derive_accuracy(result)
    };
    let Some(object) = result.as_object_mut() else {
        return;
    };
    let engine = object.entry("engine").or_insert_with(|| json!({}));
    let Some(engine) = engine.as_object_mut() else {
        return;
    };
    let rng = if sampled {
        let mut rng = serde_json::Map::new();
        for (key, name) in [
            ("generator", "family"),
            ("seed", "seed"),
            ("initial_seeds", "initial_seeds"),
            ("final_state", "final_state"),
            ("counter_stride", "counter_stride"),
            ("counter_ranges", "counter_ranges"),
            ("uniform_draws", "uniform_draws"),
        ] {
            if let Some(value) = engine.get(key) {
                rng.insert(name.into(), value.clone());
            }
        }
        J::Object(rng)
    } else {
        J::Null
    };
    // Sampled results are Monte Carlo estimates whatever their wire tags say.
    let class = if par { "limited" } else { "limited_float" };
    let accuracy = if sampled {
        json!({
            "estimate":class,"standard_uncertainty":class,
            "intervals":class,"twice_standard_errors":class,
        })
    } else {
        derived
    };
    let provenance = json!({
        "engine_id":if par { "gum-par" } else { "gum-ref" },
        "rng":rng,
        "reduction":engine.get("reduction").cloned().unwrap_or(J::Null),
        "accuracy":accuracy,
        "device":J::Null,
    });
    engine.insert("provenance".into(), provenance);
}

/// Value tags seen under one result field.
#[derive(Default)]
struct Tally {
    exact: bool,
    float: bool,
    /// Smallest guard-digit count among decimal values.
    digits: Option<u64>,
    /// A decimal value whose result carries no `guard_digits`.
    unsized_decimal: bool,
}

impl Tally {
    fn decimal(&mut self, guard: Option<u64>) {
        match guard {
            Some(g) => self.digits = Some(self.digits.map_or(g, |d| d.min(g))),
            None => self.unsized_decimal = true,
        }
    }

    /// Same precedence as `accuracy()` in lib.rs: any float, else the least
    /// decimal precision, else exact. `None` when the field holds no number.
    fn class(&self) -> Option<String> {
        if self.float {
            Some("limited_float".into())
        } else if let Some(p) = self.digits {
            Some(format!("limited_{p}"))
        } else if self.unsized_decimal {
            Some("limited".into())
        } else if self.exact {
            Some("exact".into())
        } else {
            None
        }
    }
}

/// Map each numeric result field to its PM 1004 accuracy class, read from
/// the wire tags: `{"rational"}` is exact, `{"decimal"}` is
/// `limited_<guard_digits>`, binary64 values are `limited_float`. Untagged
/// strings follow their spelling (`a/b` exact; a decimal literal is
/// `limited_<guard_digits>`, or `limited_float` when the engine states a
/// binary64 backend). Counts, symbols, names and booleans are not accuracy
/// carrying and are omitted.
fn derive_accuracy(result: &J) -> J {
    let Some(object) = result.as_object() else {
        return J::Null;
    };
    let guard = object.get("guard_digits").and_then(J::as_u64);
    let binary64 = result
        .pointer("/engine/numeric_backend")
        .and_then(J::as_str)
        .is_some_and(|backend| backend.starts_with("binary64"));
    let mut map = serde_json::Map::new();
    for (key, value) in object {
        if key == "engine" {
            continue;
        }
        let mut tally = Tally::default();
        tally_value(value, guard, binary64, &mut tally);
        if let Some(class) = tally.class() {
            map.insert(key.clone(), J::String(class));
        }
    }
    J::Object(map)
}

fn tally_value(value: &J, guard: Option<u64>, binary64: bool, tally: &mut Tally) {
    match value {
        J::Object(object) => {
            if object.contains_key("rational") {
                tally.exact = true;
            } else if object.contains_key("decimal") {
                tally.decimal(guard);
            } else if object.get("kind").and_then(J::as_str) == Some("float") {
                tally.float = true;
            } else {
                for member in object.values() {
                    tally_value(member, guard, binary64, tally);
                }
            }
        }
        J::Array(items) => {
            for item in items {
                tally_value(item, guard, binary64, tally);
            }
        }
        J::String(text) => match numeric_text(text) {
            Some(true) => tally.exact = true,
            Some(false) if binary64 => tally.float = true,
            Some(false) => tally.decimal(guard),
            None => {}
        },
        J::Number(number) if number.is_f64() => tally.float = true,
        _ => {}
    }
}

/// `Some(true)` for a rational literal `a/b`, `Some(false)` for a decimal
/// literal, `None` for any other text.
fn numeric_text(text: &str) -> Option<bool> {
    let integer = |part: &str| {
        let digits = part.strip_prefix('-').unwrap_or(part);
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    };
    if let Some((numerator, denominator)) = text.split_once('/') {
        return (integer(numerator) && integer(denominator)).then_some(true);
    }
    let decimal = text.bytes().any(|b| b.is_ascii_digit())
        && text.bytes().all(|b| b"0123456789+-.eE".contains(&b))
        && text.parse::<f64>().is_ok();
    decimal.then_some(false)
}

fn run_method(method: &str, params: &J) -> Result<J> {
    match method {
        "type_a" => statistics::type_a_request(params),
        "type_a_covariance" => statistics::covariance(params),
        "series" => statistics::series(params),
        "grouped_type_a" => statistics::grouped(params),
        "component" => propagation::component(params),
        "coverage" => coverage::evaluate(params),
        "validate_coverage" => validation::evaluate(params),
        "conformity" => conformity::evaluate(params),
        "linear_calibration" => calibration::fit(params),
        "predict_calibration" => calibration::predict(params),
        "propagate_implicit" => implicit::propagate(params),
        "correlation_matrix" => propagation::correlations(params),
        "monte_carlo" => monte_carlo::evaluate(params),
        "propagate" => propagation::independent(params),
        "analyze" => taylor::analyze(params),
        "analyze_ad" => analyze_ad(params),
        "lpu_adequacy" => lpu_adequacy(params),
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
