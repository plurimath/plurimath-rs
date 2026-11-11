//! Default/unformatted Number source procedures, Ruby0a1df68a.
//! Base is formatter input, not a request for arithmetic conversion in these writers.
use super::{el, text as xml_text, Xml};
use crate::model::Model;
use serde_json::Value;
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("numbers-data.json")).unwrap())
}
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
pub(super) fn handles(m: &Model) -> bool {
    m.class == "Math::Number"
}
pub(super) fn mini_sized(v: &Value) -> bool {
    if !v.is_object() {
        return false;
    }
    let Ok(m) = Model::from_value(v) else {
        return false;
    };
    match m.kind() {
        "Number" => truthy(m.field("mini_sub_sized")) || truthy(m.field("mini_sup_sized")),
        "Formula" => m
            .field("value")
            .as_array()
            .and_then(|a| a.first())
            .map(mini_sized)
            .unwrap_or(false),
        _ => false,
    }
}
pub(super) fn unicode_separator(m: &Model) -> &'static str {
    let values = m.field("value").as_array();
    let first_mini = values
        .and_then(|a| a.first())
        .map(mini_sized)
        .unwrap_or(false);
    let negated = values
        .and_then(|a| a.last())
        .map(|v| {
            v.get("class")
                .and_then(Value::as_str)
                .map(|c| c.starts_with("Math::Symbols::"))
                .unwrap_or(false)
                && v.get("fields")
                    .and_then(|f| f.get("value"))
                    .and_then(Value::as_str)
                    == Some("&#x338;")
        })
        .unwrap_or(false);
    if first_mini || negated {
        ""
    } else {
        " "
    }
}
pub(super) fn ordinary_string(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => {
            let s = n.to_string();
            if s.contains(['.', 'e', 'E']) {
                Err("unported promised Ruby Float#to_s Number spelling".into())
            } else {
                Ok(s)
            }
        }
        Value::Array(_) | Value::Object(_) => Err("unported promised Number Array/Hash inspection: source symbol/string element or key kinds are not retained by the common wire".into()),
    }
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if f == "unicodemath" {
        let map = if truthy(m.field("mini_sub_sized")) {
            Some("mini_sub")
        } else if truthy(m.field("mini_sup_sized")) {
            Some("mini_sup")
        } else {
            None
        };
        if let Some(map) = map {
            let value = m.field("value").as_str().ok_or_else(|| {
                "source_render_error: Number mini value has no to_sym method".to_owned()
            })?;
            // The complete source value is a digit-map key; multi-digit/missing keys return
            // nil, which the Formula/parent text interpolation renders as an empty string.
            return Ok(data()[map][value].as_str().unwrap_or("").to_owned());
        }
    }
    if let Some(formatter)=ctx.formatter { return formatter.text(m,f).map_err(|error|super::formatter_context::scope_error(ctx,error)); }
    ordinary_string(m.field("value"))
}
pub(super) fn xml(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<Vec<Xml>, String> {
    if let Some(formatter)=ctx.formatter {
        if f=="mathml" {return formatter.mathml(m).map_err(|error|super::formatter_context::scope_error(ctx,error))}
        if f=="omml" {return Ok(vec![el("m:r",vec![el("m:t",vec![xml_text(&formatter.text(m,"plain").map_err(|error|super::formatter_context::scope_error(ctx,error))?)])])])}
    }
    let value = ordinary_string(m.field("value"))?;
    Ok(vec![if f == "mathml" {
        el("mn", vec![xml_text(&value)])
    } else if f == "omml" {
        el("m:r", vec![el("m:t", vec![xml_text(&value)])])
    } else {
        return Err(format!("unknown Number XML format {f}"));
    }])
}
