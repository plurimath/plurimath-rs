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
    if m.contains_key("power_base") && m.contains_key("base_value") && m.contains_key("power_value")
    {
        if m.len() != 3 {
            return Err("unported combined source PowerBase captures".into());
        }
        return Ok(vec![node(
            "Math::Function::PowerBase",
            json!({"parameter_one":single(tx(&m["power_base"],depth+1)?),"parameter_two":unfence(single(tx(&m["base_value"],depth+1)?)),"parameter_three":unfence(single(tx(&m["power_value"],depth+1)?))}),
        )]);
    }
    if m.contains_key("power_base") && m.contains_key("power") {
        if m.keys()
            .any(|k| !["power_base", "power", "expr"].contains(&k.as_str()))
        {
            return Err("unported combined source power captures".into());
        }
        let base = single(tx(&m["power_base"], depth + 1)?);
        let script = tx(&m["power"], depth + 1)?;
        if script.len() != 1 {
            return Err("unported source sequence-power constructor profile".into());
        }
        let exponent = unfence(single(script));
        let mut out = vec![node(
            "Math::Function::Power",
            json!({"parameter_one":base,"parameter_two":exponent}),
        )];
        if let Some(v) = m.get("expr") {
            out.extend(tx(v, depth + 1)?)
        }
        return Ok(out);
    }
    if let Some(name) = m.get("unary_class").and_then(Value::as_str) {
        let class = crate::symbols::tables()["class_map"][name]
            .as_str()
            .ok_or_else(|| format!("unported source function name {name}"))?;
        let mut rest = m.clone();
        rest.remove("unary_class");
        let mut arg = if rest.is_empty() {
            Value::Null
        } else {
            single(tx(&Value::Object(rest), depth + 1)?)
        };
        if !crate::symbols::tables()["unary_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == name)
        {
            arg = unfence(arg)
        }
        let mut model = node(class, json!({"parameter_one":arg}));
        let kind = class.rsplit("::").next().unwrap_or("");
        if ["Hat", "Bar", "Vec"].contains(&kind) {
            model["fields"]["attributes"] = json!({});
        }
        if !crate::writer::unary_class(&crate::model::Model::from_value(&model)?)
            && !["Sqrt", "Abs", "Hat", "Bar", "Vec"].contains(&kind)
        {
            return Err(format!(
                "unported promised source unary constructor {class}"
            ));
        }
        return Ok(vec![model]);
    }
    if m.contains_key("number") && m.len() != 1 {
        return Err("unported source combined number capture".into());
    }
    if m.contains_key("symbol") && m.len() != 1 {
        return Err("unported source combined symbol capture".into());
    }
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
            "expr" | "sequence" | "frac" | "power_base" | "base_value" | "power_value"
            | "unary" => out.extend(tx(x, depth + 1)?),
            "intermediate_exp" => out.extend(tx(x, depth + 1)?),
            _ => return Err(format!("unported promised AsciiMath capture {k}")),
        }
    }
    Ok(out)
}
thread_local! {static CACHED_PARSER:Result<amath::Parser,String>=amath::Parser::new().map_err(|e|e.to_string());}
#[derive(Debug)]
pub enum ParseError {
    Unported(String),
    InvalidOptions(String),
    Stage(amath::Error),
    Initialization(String),
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unported(s) | Self::InvalidOptions(s) | Self::Initialization(s) => f.write_str(s),
            Self::Stage(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ParseError {}
impl From<ParseError> for String {
    fn from(e: ParseError) -> String {
        e.to_string()
    }
}
pub fn parse(input: &str, format: &str, options: &Value) -> Result<Value, ParseError> {
    if format != "asciimath" {
        return Err(ParseError::Unported(format!(
            "unported promised input format {format}"
        )));
    }
    let opts = options
        .as_object()
        .ok_or_else(|| ParseError::InvalidOptions("options must be an object".into()))?;
    if !opts.is_empty() {
        return Err(ParseError::Unported(
            "AsciiMath parse options not yet ported".into(),
        ));
    }
    let stage = CACHED_PARSER.with(|p| match p {
        Ok(parser) => parser.parse_legacy(input).map_err(ParseError::Stage),
        Err(message) => Err(ParseError::Initialization(message.clone())),
    })?;
    let tree = untag(&stage.canonical()).map_err(ParseError::Unported)?;
    let mut model = formula(tx(&tree, 0).map_err(ParseError::Unported)?);
    model["fields"]["input_string"] = json!(input);
    Ok(model)
}
pub fn response(v: &Value) -> Value {
    let Some(input) = v["input"].as_str() else {
        return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
    };
    let Some(format) = v["format"].as_str() else {
        return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"format must be a string"}});
    };
    match parse(input, format, v.get("options").unwrap_or(&json!({}))) {
        Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
        Err(e) => {
            let error = match &e {
                ParseError::Unported(_) => {
                    json!({"code":"unported_capability","message":e.to_string()})
                }
                ParseError::InvalidOptions(_) => {
                    json!({"code":"invalid_options","message":e.to_string()})
                }
                ParseError::Initialization(_) => {
                    json!({"code":"parser_initialization_error","message":e.to_string()})
                }
                ParseError::Stage(stage) => {
                    json!({"code":if stage.byte_offset().is_some(){"syntax_error"}else{"parser_error"},"message":e.to_string(),"stage":"amath","byte_offset":stage.byte_offset(),"offset_policy":"preprocessed_utf8_bytes","native_detail":format!("{stage:?}")})
                }
            };
            json!({"abi_version":1,"ok":false,"error":error})
        }
    }
}
