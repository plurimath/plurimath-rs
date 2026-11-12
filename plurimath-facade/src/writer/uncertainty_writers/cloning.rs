//! Source Uncertainty#cloned_objects and constructor validation at the line-split boundary.
use super::{truthy, Model, Value, WriterContext};
fn inspect(v: &Value) -> Result<String, String> {
    if v["class"] == "Math::Number" {
        let fields = &v["fields"];
        let mut parts = Vec::new();
        for k in ["value", "mini_sub_sized", "mini_sup_sized", "base"] {
            parts.push(format!("@{k}={}", inspect(&fields[k])?));
        }
        return Ok(format!("#<Plurimath::Math::Number {}>", parts.join(", ")));
    }
    if v.is_object() && v.get("class").is_some() {
        return Err("unported uncertainty clone invalid-object inspection".into());
    }
    crate::source_values::inspect(&crate::source_values::decode(
        v,
        crate::source_values::Domain::Literal,
        crate::source_values::Profile::Canonical,
    )?)
}
fn sym(v: &Value) -> Result<String, String> {
    if let Some(s) = v.as_str() {
        crate::source_values::inspect(&crate::source_values::RubyValue::Symbol(s.into()))
    } else {
        inspect(v)
    }
}
fn invalid(message: String) -> (String, String) {
    (
        "Plurimath::Errors::InvalidUncertainty".into(),
        format!("[plurimath] Invalid uncertainty: {message}."),
    )
}
fn decimal(s: &str, signed: bool) -> bool {
    let s = if signed {
        s.strip_prefix('-').unwrap_or(s)
    } else {
        s
    };
    let mut parts = s.split('.');
    let a = parts.next().unwrap_or("");
    if a.is_empty() || !a.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if let Some(b) = parts.next() {
        if b.is_empty() || !b.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    parts.next().is_none()
}
fn numeric(v: &Value, signed: bool) -> bool {
    v["class"] == "Math::Number"
        && v["fields"]["value"]
            .as_str()
            .is_some_and(|s| decimal(s, signed))
        && !truthy(&v["fields"]["mini_sub_sized"])
        && !truthy(&v["fields"]["mini_sup_sized"])
        && v["fields"]["base"].is_null()
}
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let (ai, af) = a.split_once('.').unwrap_or((a, ""));
    let (bi, bf) = b.split_once('.').unwrap_or((b, ""));
    let (ai, bi) = (ai.trim_start_matches('0'), bi.trim_start_matches('0'));
    let width = af.len().max(bf.len());
    ai.len()
        .cmp(&bi.len())
        .then_with(|| format!("{ai}{af:0<width$}").cmp(&format!("{bi}{bf:0<width$}")))
}
fn empty_formula(root: &Value) -> bool {
    let mut pending = vec![root];
    while let Some(v) = pending.pop() {
        if v["class"] != "Math::Formula" {
            return false;
        }
        match &v["fields"]["value"] {
            Value::Null => {}
            Value::Array(a) => pending.extend(a.iter().filter(|v| !v.is_null())),
            value => pending.push(value),
        }
    }
    true
}
fn contains_uncertainty(root: &Value) -> bool {
    let mut pending = vec![root];
    while let Some(v) = pending.pop() {
        if v["class"] == "Math::Function::Uncertainty" {
            return true;
        }
        match v {
            Value::Array(a) => pending.extend(a),
            Value::Object(_) if v["class"].is_string() => {
                if let Some(fields) = v["fields"].as_object() {
                    pending.extend(fields.values());
                }
            }
            _ => {}
        }
    }
    false
}
fn validation(m: &Model) -> Result<Option<(String, String)>, String> {
    let c = m.field("category").as_str().unwrap_or("");
    if !["standard", "expanded", "coverage_interval"].contains(&c) {
        return Ok(Some(invalid(format!(
            "unknown category {}",
            sym(m.field("category"))?
        ))));
    }
    for key in [
        "value",
        "magnitude",
        "coverage_factor",
        "coverage_probability",
        "lower",
        "upper",
    ] {
        let v = m.field(key);
        if key != "value" && v.is_null() {
            continue;
        }
        if !numeric(v, key == "value") {
            return Ok(Some(invalid(format!(
                "{key} must be a Math::Number holding a plain {}, got {}",
                if key == "value" {
                    "decimal"
                } else {
                    "unsigned decimal"
                },
                inspect(v)?
            ))));
        }
    }
    let unit = m.field("unit");
    if !unit.is_null() && unit["class"] != "Math::Function::Unitsml" {
        if unit["class"] != "Math::Formula" {
            return Ok(Some(invalid(format!(
                "unit must be a Math::Function::Unitsml, got {}",
                inspect(unit)?
            ))));
        }
        if empty_formula(unit) {
            return Ok(Some(invalid("unit must not be empty".into())));
        }
        if contains_uncertainty(unit) {
            return Ok(Some(invalid("unit must not contain an uncertainty".into())));
        }
    }
    if c == "standard"
        && !m.field("distribution").is_null()
        && !matches!(
            m.field("distribution").as_str(),
            Some("rect" | "triang" | "arcsine")
        )
    {
        return Ok(Some(invalid(format!(
            "unknown distribution {}",
            sym(m.field("distribution"))?
        ))));
    }
    let forbidden: &[&str] = match c {
        "standard" => &[
            "coverage_factor",
            "coverage_probability",
            "lower",
            "upper",
            "interval_kind",
        ],
        "expanded" => &["lower", "upper", "interval_kind", "distribution"],
        _ => &["magnitude", "coverage_factor", "distribution"],
    };
    for key in forbidden {
        if !m.field(key).is_null() {
            return Ok(Some(invalid(format!(
                "{key} does not apply to a {c} uncertainty"
            ))));
        }
    }
    let notation = if truthy(m.field("notation")) {
        m.field("notation").clone()
    } else {
        Value::String("absolute".into())
    };
    if c != "standard" && notation != "absolute" {
        return Ok(Some(invalid(format!(
            "notation {} applies only to a standard uncertainty",
            sym(&notation)?
        ))));
    }
    let required: &[&str] = match c {
        "standard" => &["magnitude"],
        "expanded" => &["magnitude", "coverage_factor"],
        _ => &["lower", "upper", "coverage_probability"],
    };
    for key in required {
        if m.field(key).is_null() {
            return Ok(Some(invalid(format!(
                "{key} is required for a {c} uncertainty"
            ))));
        }
    }
    if c == "standard" {
        if notation != "absolute" && notation != "concise" {
            return Ok(Some(invalid(format!(
                "unknown notation {}",
                sym(&notation)?
            ))));
        }
        if notation == "concise" {
            let v = super::n(m, "value")?;
            let mag = super::n(m, "magnitude")?;
            let places = |s: &str| s.split_once('.').map_or(0, |(_, f)| f.len());
            if places(&v) != places(&mag) {
                return Ok(Some(invalid(format!("magnitude {mag} cannot be written concisely after {v}: it needs as many decimal places as the value"))));
            }
        }
    } else {
        if c == "expanded" {
            let factor = super::n(m, "coverage_factor")?;
            if !factor.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
                return Ok(Some(invalid(format!(
                    "coverage factor {factor} must be greater than 0"
                ))));
            }
        }
        if c == "coverage_interval"
            && !m.field("interval_kind").is_null()
            && !matches!(
                m.field("interval_kind").as_str(),
                Some("shortest" | "probabilistically_symmetric")
            )
        {
            return Ok(Some(invalid(format!(
                "unknown interval kind {}",
                sym(m.field("interval_kind"))?
            ))));
        }
        if !m.field("coverage_probability").is_null() {
            let prob = super::n(m, "coverage_probability")?;
            if !prob.bytes().any(|b| (b'1'..=b'9').contains(&b))
                || compare(&prob, "1") != std::cmp::Ordering::Less
            {
                return Ok(Some(invalid(format!(
                    "coverage probability {prob} must lie strictly between 0 and 1"
                ))));
            }
        }
        if c == "coverage_interval" {
            let lower = super::n(m, "lower")?;
            let upper = super::n(m, "upper")?;
            if compare(&lower, &upper) != std::cmp::Ordering::Less {
                return Ok(Some(invalid(format!(
                    "lower endpoint {lower} is not below upper endpoint {upper}"
                ))));
            }
        }
    }
    Ok(None)
}
fn error(class: &str, message: String, f: &str, ctx: &WriterContext) -> String {
    if f == "omml" {
        super::source_error(format!("source_render_error: {message}"), ctx)
    } else {
        let direct = WriterContext {
            formula_input: None,
            ..*ctx
        };
        super::super::formatter_context::source_error(&direct, class, message)
    }
}
pub(super) fn before_split(root: &Value, f: &str, ctx: &WriterContext) -> Result<(), String> {
    enum Step<'a> {
        Visit(&'a Value),
        CloneSlot(&'a str, &'a Value),
        Validate(&'a Value),
    }
    let mut pending = vec![Step::Visit(root)];
    while let Some(step) = pending.pop() {
        match step {
            Step::CloneSlot(key, field) => {
                if (key == "value" && field.is_null()) || field == false {
                    return Err(error(
                        "NoMethodError",
                        format!(
                            "undefined method 'cloned_objects' for {}",
                            if field.is_null() { "nil" } else { "false" }
                        ),
                        f,
                        ctx,
                    ));
                }
                pending.push(Step::Visit(field));
            }
            Step::Validate(v) => {
                if let Some((class, message)) = validation(&Model::from_value(v)?)? {
                    return Err(error(&class, message, f, ctx));
                }
            }
            Step::Visit(v) => {
                if v["class"] == "Math::Function::Uncertainty" {
                    pending.push(Step::Validate(v));
                    for key in [
                        "value",
                        "magnitude",
                        "coverage_factor",
                        "coverage_probability",
                        "lower",
                        "upper",
                        "unit",
                    ]
                    .into_iter()
                    .rev()
                    {
                        pending.push(Step::CloneSlot(key, &v["fields"][key]));
                    }
                    continue;
                }
                match v {
                    Value::Array(a) => pending.extend(a.iter().rev().map(Step::Visit)),
                    Value::Object(_) => {
                        if let Some(class) = v["class"].as_str() {
                            if class == "Math::Number" || class.starts_with("Math::Symbols::") {
                                continue;
                            }
                            if let Some(fields) = v["fields"].as_object() {
                                for (key, value) in fields.iter().rev() {
                                    if !["options", "attributes", "input_string", "source_values"]
                                        .contains(&key.as_str())
                                    {
                                        pending.push(Step::Visit(value));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
