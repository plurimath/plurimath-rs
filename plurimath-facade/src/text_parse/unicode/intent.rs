//! Unicode intent/argument rules from the pinned Ruby mapper and marker reader.
use super::{incomplete, keys, simple, Rec};
use crate::model::{formula, node};
use serde_json::{json, Map, Value};

fn invalid(message: impl ToString) -> Value {
    json!({"code":"PARSE_ERROR", "source_class":"Plurimath::Math::ParseError",
        "cause_class":"Plurimath::Errors::InvalidUncertainty", "message":message.to_string()})
}

fn marker_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

/// Ruby's marker name boundary uses ASCII word characters and hyphen.
fn marker_name(text: &str) -> Option<(&'static str, &str)> {
    let text = text.trim_start_matches(marker_space);
    for name in [
        "standard-uncertainty",
        "expanded-uncertainty",
        "coverage-interval",
    ] {
        if let Some(rest) = text.strip_prefix(name) {
            if !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Some((name, rest));
            }
        }
    }
    None
}

fn unit_content(content: &Value) -> Result<Value, Value> {
    let values = if content["class"] == "Math::Formula" {
        content["fields"]["value"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        vec![content.clone()]
    };
    let mut units = values.into_iter().filter(|value| {
        value["class"] == "Math::Function::Arg"
            && value["fields"]["parameter_two"]["class"] == "Math::Symbols::Symbol"
            && value["fields"]["parameter_two"]["fields"]["value"] == "unit"
    });
    let first = units
        .next()
        .map(|value| value["fields"]["parameter_one"].clone());
    if units.next().is_some() {
        return Err(invalid("more than one unit argument"));
    }
    match first.filter(|value| !value.is_null()) {
        Some(value) if value["class"] == "Math::Formula" => Ok(value),
        Some(value) => Ok(formula(vec![value])),
        None => Err(invalid(
            "marker refers to $unit but no unit content was found",
        )),
    }
}

fn intent_or_uncertainty(content: Value, intent: Value) -> Result<Value, Value> {
    let text = if intent["class"] == "Math::Function::Text" {
        intent["fields"]["parameter_one"].as_str()
    } else {
        intent.as_str()
    };
    let Some((name, rest)) = text.and_then(marker_name) else {
        return Ok(node(
            "Math::Function::Intent",
            json!({"parameter_one":content,"parameter_two":intent}),
        ));
    };
    let rest = rest.trim_matches(marker_space);
    let (property, application) = if let Some(property) = rest.strip_prefix(':') {
        let at = property
            .find('(')
            .ok_or_else(|| invalid("marker has no arguments"))?;
        let name = property[..at].trim_end_matches(marker_space);
        if name.is_empty() || name.chars().any(|c| marker_space(c) || ":,)".contains(c)) {
            return Err(invalid("invalid marker property"));
        }
        (Some(name), &property[at..])
    } else {
        (None, rest)
    };
    let arguments = application
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .filter(|s| !s.contains(['(', ')']))
        .ok_or_else(|| invalid("marker is not name[:property](arguments)"))?;
    let mut arguments: Vec<&str> = arguments
        .split(',')
        // String#strip accepts NUL as well as Ruby's ASCII whitespace set.
        .map(|s| s.trim_matches(|c| c == '\0' || marker_space(c)))
        .collect();
    let unit = arguments.last() == Some(&"$unit");
    if unit {
        arguments.pop();
    }
    if arguments.iter().any(|a| a.is_empty() || a.starts_with('$')) {
        return Err(invalid(
            "marker arguments must be numbers; $unit is only allowed last",
        ));
    }
    let (category, fields): (&str, &[&str]) = match (name, arguments.len()) {
        ("standard-uncertainty", 2) => ("ustd", &["value", "magnitude"]),
        ("expanded-uncertainty", 3) => ("uexp", &["value", "magnitude", "coverage_factor"]),
        ("expanded-uncertainty", 4) => (
            "uexp",
            &[
                "value",
                "magnitude",
                "coverage_factor",
                "coverage_probability",
            ],
        ),
        ("coverage-interval", 4) => ("ucov", &["value", "lower", "upper", "coverage_probability"]),
        _ => return Err(invalid("wrong number of marker arguments")),
    };
    let concise = match (category, property) {
        (_, None) => false,
        ("ustd", Some("concise")) => true,
        ("ucov", Some("shortest" | "probabilistically-symmetric")) => false,
        _ => return Err(invalid("unknown marker property")),
    };
    let mut values: Map<String, Value> = fields
        .iter()
        .zip(arguments)
        .map(|(k, v)| ((*k).into(), json!(v)))
        .collect();
    values.insert("uncertainty_class".into(), json!(category));
    if category == "ucov" {
        if let Some(property) = property {
            values.insert(
                "interval_kind".into(),
                json!(if property == "shortest" {
                    "shortest"
                } else {
                    "symmetric"
                }),
            );
        }
    }
    // Use the existing decimal and uncertainty validation shared with PMA.
    let bound_unit = if unit {
        Some(unit_content(&content)?)
    } else {
        None
    };
    let mut uncertainty = crate::pma_parse::uncertainty_node(values)?;
    if concise {
        uncertainty["fields"]["notation"] = json!("concise");
    }
    if let Some(unit) = bound_unit {
        uncertainty["fields"]["unit"] = unit;
    }
    Ok(uncertainty)
}

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if keys(m, &["intent", "intent_expr"]) {
        let expression = simple(rec(&m["intent_expr"])?, "intent expression")?;
        let mut values = expression["fields"]["parameter_two"]
            .as_array()
            .cloned()
            .ok_or_else(|| incomplete("intent expression is not a fenced list"))?;
        if values.is_empty() {
            return Err(incomplete("intent expression has no label"));
        }
        let label = values.remove(0);
        return intent_or_uncertainty(super::filter_values(values), label)
            .map(|value| Some(vec![value]));
    }
    if keys(m, &["intent", "intent_arguments", "first_value"])
        || keys(
            m,
            &[
                "intent",
                "open_paren",
                "intent_arguments",
                "first_value",
                "close_paren",
            ],
        )
    {
        let content = super::filter_values(rec(&m["first_value"])?);
        let label = simple(rec(&m["intent_arguments"])?, "intent label")?;
        return intent_or_uncertainty(content, label).map(|value| Some(vec![value]));
    }
    if keys(m, &["arg", "arg_arguments", "first_value"]) {
        let values = rec(&m["first_value"])?;
        // A source sequence always stays a Formula, including a one-item list.
        let content = if m["first_value"].is_array() {
            formula(values)
        } else {
            super::filter_values(values)
        };
        let label = simple(rec(&m["arg_arguments"])?, "argument label")?;
        return Ok(Some(vec![node(
            "Math::Function::Arg",
            json!({"parameter_one":content,"parameter_two":label}),
        )]));
    }
    if keys(m, &["arg"]) || keys(m, &["arg", "arg_arguments"]) || keys(m, &["arg", "first_value"]) {
        return Err(
            json!({"code":"PARSE_ERROR","source_class":"Plurimath::Math::ParseError","cause_class":"ArgumentError","message":"argument needs a name and content"}),
        );
    }
    Ok(None)
}
