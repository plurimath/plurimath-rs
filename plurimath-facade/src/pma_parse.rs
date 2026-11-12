//! PMA (dialect P) input for the native facade.
//!
//! Source: plurimath-ruby 0a1df68 `Plurimath::Pma#to_formula` ->
//! `Asciimath::Parser.new(text, dialect: :pma)` -> `Amath.parse(text, dialect: :pma)` ->
//! `Asciimath::Mapper#pma_formula`. The typed `PmaDocument` comes from the producer's
//! own port, amath-rs `amath::pma::parse_model` (public 16195ed, the Rust port of
//! `Amath::Pma`); no grammar, heuristics or fixtures live here. This module only maps
//! the typed term to the Formula the Ruby mapper builds:
//!
//! * measured value -> `Rules::UncertaintyTree.node(pma_uncertainty(term))`, i.e. a
//!   `Math::Function::Uncertainty` validated as its Ruby constructor validates it;
//! * unit -> `Math::Function::Unitsml.new(literal)` (the quantity assertion is checked by
//!   the producer and not carried in the node, as in Ruby);
//! * `diff(q)` -> refused ("has no Plurimath node yet").
//!
//! Every refusal is the public `Plurimath::Math::ParseError`; producer refusals also carry
//! the PMA strict-taxonomy `category` and caller-input byte/char offsets.
use crate::model::{formula, node, number};
use amath::pma::{self, PmaError};
use serde_json::{json, Map, Value};

const SOURCE_CLASS: &str = "Plurimath::Math::ParseError";

fn parse_error(message: &str) -> Value {
    json!({"code":"PARSE_ERROR","source_class":SOURCE_CLASS,"message":message})
}

fn producer_error(error: PmaError) -> Value {
    match error {
        PmaError::Refused(r) => json!({
            "code":"PARSE_ERROR","source_class":SOURCE_CLASS,"message":r.to_string(),
            "category":r.category,"byte_offset":r.byte_offset,"char_offset":r.char_offset,
            "offset_policy":"caller_utf8_bytes"
        }),
        PmaError::Resource(e) => match e {
            parsanol_stage::PargError::CallDepthLimitExceeded { depth, max_depth } => {
                json!({"code":"RESOURCE_LIMIT","category":"call_depth_limit","depth":depth,"max_depth":max_depth})
            }
            parsanol_stage::PargError::RecursionLimitExceeded { depth, max_depth } => {
                json!({"code":"RESOURCE_LIMIT","category":"recursion_limit","depth":depth,"max_depth":max_depth})
            }
            other => json!({"code":"RESOURCE_LIMIT","message":other.to_string()}),
        },
        other => json!({"code":"INCOMPLETE_PMA_TRANSLATION","message":other.to_string()}),
    }
}

pub fn parse(input: &str, options: &Value) -> Result<Value, Value> {
    if options.as_object().is_some_and(|o| !o.is_empty()) {
        return Err(
            json!({"code":"PARSE_OPTION_ERROR","message":"PMA version1 accepts no locale or decimal parse options"}),
        );
    }
    // Ruby: pma numbers always use the full stop, whatever the locale.
    let document = pma::parse_model(input, ".").map_err(producer_error)?;
    let portable: Value =
        serde_json::from_str(&pma::to_portable_json(&document).map_err(producer_error)?)
            .map_err(|e| json!({"code":"INCOMPLETE_PMA_TRANSLATION","message":e.to_string()}))?;
    let term = &portable["term"];
    let value = if let Some(u) = term.get("uncertainty") {
        uncertainty_node(pma_uncertainty(u)?)?
    } else if let Some(unit) = term.get("unit") {
        unitsml(literal(unit)?)
    } else {
        return Err(parse_error(
            "diff(q) has no Plurimath node yet (docs/pma.adoc, open decision 9)",
        ));
    };
    let mut model = formula(vec![value]);
    model["fields"]["input_string"] = json!(input);
    Ok(model)
}

fn unitsml(text: &str) -> Value {
    node("Math::Function::Unitsml", json!({"text": text}))
}

fn literal(unit: &Value) -> Result<&str, Value> {
    unit["literal"]
        .as_str()
        .ok_or_else(|| parse_error("PMA unit term without a literal"))
}

/// Mapper#pma_uncertainty: the fields as dialect L names them.
fn pma_uncertainty(model: &Value) -> Result<Map<String, Value>, Value> {
    let mut fields: Map<String, Value> = model
        .as_object()
        .ok_or_else(|| parse_error("PMA uncertainty is not an object"))?
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if fields.remove("plus_minus").is_some() {
        fields.insert("uncertainty_class".into(), json!("ustd"));
    }
    if let Some(unit) = fields.get("unit").cloned() {
        fields.insert("unit".into(), json!(literal(&unit)?));
    }
    if let Some(relative) = fields.remove("relative_magnitude") {
        let value = text_of(&fields, "value")?;
        let percent = relative
            .as_str()
            .ok_or_else(|| parse_error("relative magnitude is not text"))?;
        fields.insert(
            "magnitude".into(),
            json!(relative_magnitude(&value, percent)?),
        );
    }
    for (k, v) in fields.iter_mut() {
        if !v.is_string() {
            return Err(parse_error(&format!(
                "expected a hash of text fields, got {k}: {v}"
            )));
        }
    }
    Ok(fields)
}

fn text_of(fields: &Map<String, Value>, name: &str) -> Result<String, Value> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| parse_error(&format!("{name} is required")))
}

// ---- exact decimal text arithmetic (BigDecimal#to_s("F") semantics) ----

/// (negative, digits, scale): value = digits * 10^-scale.
fn decimal(text: &str) -> Option<(bool, String, usize)> {
    let (neg, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, frac) = body.split_once('.').unwrap_or((body, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((neg, format!("{whole}{frac}"), frac.len()))
}

fn multiply(a: &str, b: &str) -> String {
    let (x, y): (Vec<u32>, Vec<u32>) = (
        a.bytes().rev().map(|c| (c - b'0') as u32).collect(),
        b.bytes().rev().map(|c| (c - b'0') as u32).collect(),
    );
    let mut out = vec![0u32; x.len() + y.len() + 1];
    for (i, dx) in x.iter().enumerate() {
        let mut carry = 0;
        for (j, dy) in y.iter().enumerate() {
            let t = out[i + j] + dx * dy + carry;
            out[i + j] = t % 10;
            carry = t / 10;
        }
        let mut k = i + y.len();
        while carry > 0 {
            let t = out[k] + carry;
            out[k] = t % 10;
            carry = t / 10;
            k += 1;
        }
    }
    let s: String = out
        .iter()
        .rev()
        .map(|d| char::from(b'0' + *d as u8))
        .collect();
    let s = s.trim_start_matches('0');
    if s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// digits * 10^-scale rendered as BigDecimal#to_s("F").delete_suffix(".0").
fn render(digits: &str, scale: usize) -> String {
    let padded = format!("{:0>width$}", digits, width = scale + 1);
    let (whole, frac) = padded.split_at(padded.len() - scale);
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac}")
    }
}

/// Mapper#relative_magnitude: |value| * percent / 100, exact on the decimal text.
fn relative_magnitude(value: &str, percent: &str) -> Result<String, Value> {
    let (_, vd, vs) =
        decimal(value).ok_or_else(|| parse_error(&format!("invalid value {value:?}")))?;
    let (_, pd, ps) =
        decimal(percent).ok_or_else(|| parse_error(&format!("invalid percent {percent:?}")))?;
    Ok(render(&multiply(&vd, &pd), vs + ps + 2))
}

// ---- Rules::UncertaintyTree.node ----

const MAX_SCALED_DECIMAL_LENGTH: usize = 1_000_000;

fn invalid(detail: &str) -> Value {
    // Errors::InvalidUncertainty, re-raised as Math::ParseError at the public boundary.
    json!({"code":"PARSE_ERROR","source_class":SOURCE_CLASS,"cause_class":"Plurimath::Errors::InvalidUncertainty","message":detail})
}

fn allowed(category: &str) -> &'static [&'static str] {
    match category {
        "standard" => &[
            "value",
            "magnitude",
            "concise_digits",
            "unit",
            "distribution",
        ],
        "expanded" => &[
            "value",
            "magnitude",
            "coverage_factor",
            "coverage_probability",
            "unit",
        ],
        _ => &[
            "value",
            "lower",
            "upper",
            "coverage_probability",
            "interval_kind",
            "unit",
        ],
    }
}

fn scale_number(text: &str, power: i64) -> Result<String, Value> {
    if power == 0 {
        return Ok(text.to_string());
    }
    let (sign, body) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (body.contains('.') && fraction.is_empty())
    {
        return Err(invalid(&format!("invalid scaled decimal {text:?}")));
    }
    let digits = format!("{whole}{fraction}");
    let point = whole.len() as i64 + power;
    let n = digits.len() as i64;
    let length = if point <= 0 {
        2 - point + n
    } else if point >= n {
        point
    } else {
        n + 1
    };
    if (length as usize) + sign.len() > MAX_SCALED_DECIMAL_LENGTH {
        return Err(invalid(&format!(
            "shared exponent expands beyond {MAX_SCALED_DECIMAL_LENGTH} decimal characters"
        )));
    }
    let scaled = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point >= n {
        format!("{}{}", digits, "0".repeat((point - n) as usize))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    // text.sub(/\A0+(?=[0-9])/, "")
    let trimmed = scaled.trim_start_matches('0');
    let trimmed = if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        trimmed
    } else {
        &scaled[scaled.len() - trimmed.len() - 1..]
    };
    Ok(format!("{sign}{trimmed}"))
}

fn uncertainty_node(mut fields: Map<String, Value>) -> Result<Value, Value> {
    let class = fields
        .remove("uncertainty_class")
        .and_then(|v| v.as_str().map(str::to_string));
    let category = match class.as_deref() {
        Some("ustd") => "standard",
        Some("uexp") => "expanded",
        Some("ucov") => "coverage_interval",
        _ => return Err(invalid("unknown uncertainty class")),
    };
    let exponent = fields
        .remove("shared_exponent")
        .and_then(|v| v.as_str().map(str::to_string));
    let unknown: Vec<&String> = fields
        .keys()
        .filter(|k| !allowed(category).contains(&k.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(invalid(&format!(
            "unexpected {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if fields.contains_key("concise_digits") {
        // PMA refuses concise notation in the producer (category "concise"); never reached.
        return Err(invalid("concise digits are not PMA input"));
    }
    let get = |k: &str| fields.get(k).and_then(Value::as_str).map(str::to_string);
    let mut value = get("value");
    let mut magnitude = get("magnitude");
    let mut lower = get("lower");
    let mut upper = get("upper");
    if let Some(exponent) = exponent {
        let ok = {
            let e = exponent.strip_prefix(['+', '-']).unwrap_or(&exponent);
            !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit())
        };
        if !ok {
            return Err(invalid(&format!("invalid shared exponent {exponent:?}")));
        }
        let power: i64 = exponent
            .parse()
            .map_err(|_| invalid(&format!("invalid shared exponent {exponent:?}")))?;
        if let Some(v) = &value {
            value = Some(scale_number(v, power)?);
        }
        for f in [&mut magnitude, &mut lower, &mut upper] {
            if let Some(v) = f.as_ref() {
                *f = Some(scale_number(v, power)?);
            }
        }
    }
    let interval_kind = match get("interval_kind").as_deref() {
        None => None,
        Some("shortest") => Some("shortest"),
        Some("symmetric") => Some("probabilistically_symmetric"),
        Some(other) => return Err(invalid(&format!("unknown interval kind {other:?}"))),
    };
    let coverage_factor = get("coverage_factor");
    let coverage_probability = get("coverage_probability");
    let distribution = get("distribution");
    let unit = get("unit");
    validate(
        category,
        &value,
        &magnitude,
        &coverage_factor,
        &coverage_probability,
        &lower,
        &upper,
        interval_kind,
        &distribution,
    )?;
    let num = |v: &Option<String>| v.as_deref().map(number).unwrap_or(Value::Null);
    Ok(node(
        "Math::Function::Uncertainty",
        json!({
            "category": category,
            "coverage_factor": num(&coverage_factor),
            "coverage_probability": num(&coverage_probability),
            "distribution": distribution,
            "interval_kind": interval_kind,
            "lower": num(&lower),
            "magnitude": num(&magnitude),
            "notation": if category == "standard" { json!("absolute") } else { Value::Null },
            "unit": unit.as_deref().map(unitsml).unwrap_or(Value::Null),
            "upper": num(&upper),
            "value": num(&value),
        }),
    ))
}

// ---- Math::Function::Uncertainty#validate! ----

fn plain_decimal(text: &str, signed: bool) -> bool {
    let body = if signed {
        text.strip_prefix('-').unwrap_or(text)
    } else {
        text
    };
    let (whole, frac) = body
        .split_once('.')
        .map(|(w, f)| (w, Some(f)))
        .unwrap_or((body, None));
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// Compare two non-negative/signed plain decimals exactly.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let (an, ad, as_) = decimal(a).unwrap();
    let (bn, bd, bs) = decimal(b).unwrap();
    let scale = as_.max(bs);
    let pad = |d: &str, s: usize| format!("{d}{}", "0".repeat(scale - s));
    let (x, y) = (pad(&ad, as_), pad(&bd, bs));
    let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
    let mag = x.len().cmp(&y.len()).then(x.cmp(y));
    let zero = |s: &str| s.is_empty();
    let an = an && !zero(x);
    let bn = bn && !zero(y);
    match (an, bn) {
        (false, true) => Greater,
        (true, false) => Less,
        (false, false) => mag,
        (true, true) => mag.reverse(),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate(
    category: &str,
    value: &Option<String>,
    magnitude: &Option<String>,
    coverage_factor: &Option<String>,
    coverage_probability: &Option<String>,
    lower: &Option<String>,
    upper: &Option<String>,
    interval_kind: Option<&str>,
    distribution: &Option<String>,
) -> Result<(), Value> {
    match value {
        Some(v) if plain_decimal(v, true) => {}
        other => {
            return Err(invalid(&format!(
                "value must be a Math::Number holding a plain decimal, got {other:?}"
            )))
        }
    }
    for (name, field) in [
        ("magnitude", magnitude),
        ("coverage_factor", coverage_factor),
        ("coverage_probability", coverage_probability),
        ("lower", lower),
        ("upper", upper),
    ] {
        if let Some(v) = field {
            if !plain_decimal(v, false) {
                return Err(invalid(&format!(
                    "{name} must be a Math::Number holding a plain unsigned decimal, got {v:?}"
                )));
            }
        }
    }
    let forbid = |pairs: &[(&str, bool)]| -> Result<(), Value> {
        for (name, set) in pairs {
            if *set {
                return Err(invalid(&format!(
                    "{name} does not apply to a {category} uncertainty"
                )));
            }
        }
        Ok(())
    };
    let require = |pairs: &[(&str, bool)]| -> Result<(), Value> {
        for (name, set) in pairs {
            if !*set {
                return Err(invalid(&format!(
                    "{name} is required for a {category} uncertainty"
                )));
            }
        }
        Ok(())
    };
    let probability = |p: &Option<String>| -> Result<(), Value> {
        if let Some(p) = p {
            if !(compare(p, "0") == std::cmp::Ordering::Greater
                && compare(p, "1") == std::cmp::Ordering::Less)
            {
                return Err(invalid(&format!(
                    "coverage probability {p} must lie strictly between 0 and 1"
                )));
            }
        }
        Ok(())
    };
    match category {
        "standard" => {
            if let Some(d) = distribution {
                if !["rect", "triang", "arcsine"].contains(&d.as_str()) {
                    return Err(invalid(&format!("unknown distribution :{d}")));
                }
            }
            forbid(&[
                ("coverage_factor", coverage_factor.is_some()),
                ("coverage_probability", coverage_probability.is_some()),
                ("lower", lower.is_some()),
                ("upper", upper.is_some()),
                ("interval_kind", interval_kind.is_some()),
            ])?;
            require(&[("magnitude", magnitude.is_some())])?;
        }
        "expanded" => {
            forbid(&[
                ("lower", lower.is_some()),
                ("upper", upper.is_some()),
                ("interval_kind", interval_kind.is_some()),
                ("distribution", distribution.is_some()),
            ])?;
            require(&[
                ("magnitude", magnitude.is_some()),
                ("coverage_factor", coverage_factor.is_some()),
            ])?;
            let k = coverage_factor.as_deref().unwrap();
            if compare(k, "0") != std::cmp::Ordering::Greater {
                return Err(invalid(&format!(
                    "coverage factor {k} must be greater than 0"
                )));
            }
            probability(coverage_probability)?;
        }
        _ => {
            forbid(&[
                ("magnitude", magnitude.is_some()),
                ("coverage_factor", coverage_factor.is_some()),
                ("distribution", distribution.is_some()),
            ])?;
            require(&[
                ("lower", lower.is_some()),
                ("upper", upper.is_some()),
                ("coverage_probability", coverage_probability.is_some()),
            ])?;
            probability(coverage_probability)?;
            let (l, u) = (lower.as_deref().unwrap(), upper.as_deref().unwrap());
            if compare(l, u) != std::cmp::Ordering::Less {
                return Err(invalid(&format!(
                    "lower endpoint {l} is not below upper endpoint {u}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_and_scaled_decimals_follow_the_ruby_source() {
        assert_eq!(relative_magnitude("10.3", "0.5").unwrap(), "0.0515");
        assert_eq!(relative_magnitude("10.3", "2").unwrap(), "0.206");
        assert_eq!(relative_magnitude("-1.5", "200").unwrap(), "3");
        assert_eq!(scale_number("3.85", 4).unwrap(), "38500");
        assert_eq!(scale_number("1.024", -3).unwrap(), "0.001024");
        assert_eq!(scale_number("0.003", -3).unwrap(), "0.000003");
        assert_eq!(scale_number("-0.23", 1).unwrap(), "-2.3");
    }
    #[test]
    fn constructor_refusals_follow_uncertainty_validate() {
        assert!(parse("uexp(1.0, 0.3, 0, 0.95)", &json!({})).is_err());
        assert!(parse("diff((5.0 +/- 0.1) unit(K))", &json!({})).is_err());
        let m = parse("(10.3 +/- 0.5%)", &json!({})).unwrap();
        assert_eq!(
            m["fields"]["value"][0]["fields"]["magnitude"]["fields"]["value"],
            "0.0515"
        );
    }
}
