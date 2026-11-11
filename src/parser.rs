//! Source capture transformation over the accepted stage parser; no text grammar here.
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};
fn untag(v: &Value) -> Result<Value, String> {
    if let Some(s) = v.get("slice") {
        return Ok(s.clone());
    }
    if let Some(a) = v.get("array").and_then(Value::as_array) {
        return a
            .iter()
            .map(untag)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    if let Some(a) = v.get("hash").and_then(Value::as_array) {
        let mut m = Map::new();
        for pair in a {
            let key = pair[0]["sym"].as_str().ok_or("invalid stage capture key")?;
            m.insert(key.into(), untag(&pair[1])?);
        }
        return Ok(Value::Object(m));
    }
    if let Some(a) = v.as_array() {
        return a
            .iter()
            .map(untag)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    Ok(v.clone())
}
fn single(mut a: Vec<Value>) -> Value {
    if a.len() == 1 {
        a.pop().unwrap()
    } else {
        formula(a)
    }
}
fn unfence(v: Value) -> Value {
    if v["class"] == "Math::Function::Fenced" {
        if let Some(a) = v["fields"]["parameter_two"].as_array() {
            return single(a.clone());
        }
    }
    v
}
fn tx(v: &Value, depth: usize) -> Result<Vec<Value>, String> {
    if depth > 128 {
        return Err("common converter depth limit128".into());
    }
    let m = v
        .as_object()
        .ok_or_else(|| format!("unported stage capture value {v}"))?;
    if let Some(n) = m.get("number") {
        return Ok(vec![number(
            n.as_str().ok_or("number capture is not source text")?,
        )]);
    }
    if let Some(s) = m.get("symbol") {
        return Ok(vec![crate::symbols::symbol(
            s.as_str().ok_or("symbol capture is not source text")?,
        )]);
    }
    if m.contains_key("numerator") && m.contains_key("denominator") {
        let a = unfence(single(tx(&m["numerator"], depth + 1)?));
        let b = unfence(single(tx(&m["denominator"], depth + 1)?));
        return Ok(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":a,"parameter_two":b}),
        )]);
    }
    if m.get("binary_class").and_then(Value::as_str) == Some("frac") {
        let a = unfence(single(tx(&m["base_value"], depth + 1)?));
        let b = unfence(single(tx(&m["power_value"], depth + 1)?));
        return Ok(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":a,"parameter_two":b}),
        )]);
    }
    if m.contains_key("lparen") && m.contains_key("rparen") {
        let a = crate::symbols::symbol(m["lparen"].as_str().ok_or("fence open must be text")?);
        let b = crate::symbols::symbol(m["rparen"].as_str().ok_or("fence close must be text")?);
        let body = tx(&m["expr"], depth + 1)?;
        return Ok(vec![node(
            "Math::Function::Fenced",
            json!({"options":{},"parameter_one":a,"parameter_two":body,"parameter_three":b}),
        )]);
    }
    // Fenced operands are unwrapped only at explicit fraction argument positions in source;
    // general grouping remains an unported case until its full source node is available.
    let mut out = vec![];
    for (k, x) in m {
        match k.as_str() {
            "expr" | "sequence" | "frac" | "power_base" | "base_value" | "power_value" => {
                out.extend(tx(x, depth + 1)?)
            }
            "intermediate_exp" => out.extend(tx(x, depth + 1)?),
            _ => return Err(format!("unported promised AsciiMath capture {k}")),
        }
    }
    Ok(out)
}
pub fn parse(input: &str, format: &str, options: &Value) -> Result<Value, String> {
    if format != "asciimath" {
        return Err(format!("unported promised input format {format}"));
    }
    if options
        .as_object()
        .ok_or("options must be an object")?
        .len()
        != 0
    {
        return Err("AsciiMath parse options not yet ported".into());
    }
    let stage = crate::stage_tree(input)?;
    let tree = untag(&stage)?;
    let mut model = formula(tx(&tree, 0)?);
    model["fields"]["input_string"] = json!(input);
    Ok(model)
}
