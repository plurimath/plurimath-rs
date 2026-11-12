//! Port of the original HTML Transform rules over source-normalized captures.
//! The declaration catalog contains lexical/class defaults, never input oracles.
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

fn declarations() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/html-parse/source-declarations.json"
        ))
        .expect("checked HTML source declarations")
    })
}
fn error(message: impl Into<String>) -> Value {
    json!({"code":"INCOMPLETE_HTML_TRANSFORM","message":message.into()})
}
fn core(v: &Value) -> bool {
    v.get("class")
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with("Math::"))
        && v.get("fields").is_some_and(Value::is_object)
}
fn simple(v: &Value) -> bool {
    !v.is_array() && (!v.is_object() || core(v))
}
fn sequence(v: &Value) -> bool {
    v.as_array().is_some_and(|xs| xs.iter().all(simple))
}
fn matches(rule: &Value, object: &Map<String, Value>) -> bool {
    let Some(bindings) = rule["bindings"].as_object() else {
        return false;
    };
    bindings.len() == object.len()
        && bindings.iter().all(|(key, kind)| {
            object.get(key).is_some_and(|value| {
                if kind == "simple" {
                    simple(value)
                } else {
                    sequence(value)
                }
            })
        })
}
fn list(v: &Value) -> Vec<Value> {
    if let Some(xs) = v.as_array() {
        xs.clone()
    } else {
        vec![v.clone()]
    }
}
fn append(first: Value, tail: &Value) -> Value {
    let mut values = vec![first];
    values.extend(list(tail));
    Value::Array(values)
}
fn flatten(value: &Value, out: &mut Vec<Value>) {
    if let Some(xs) = value.as_array() {
        for x in xs {
            flatten(x, out)
        }
    } else if !value.is_null() {
        out.push(value.clone())
    }
}
fn filter(value: &Value) -> Value {
    let mut out = Vec::new();
    if value.is_array() {
        flatten(value, &mut out)
    } else if value["class"] == "Math::Formula" {
        out = list(&value["fields"]["value"])
    } else {
        return value.clone();
    }
    match out.len() {
        0 => Value::Null,
        1 => out.remove(0),
        _ => formula(out),
    }
}
fn function(name: &str, args: &[Value]) -> Result<Value, Value> {
    let mut result = declarations()["functions"][name].clone();
    if !core(&result) {
        return Err(error(format!(
            "source HTML function constructor {name} unavailable"
        )));
    }
    for (slot, value) in ["parameter_one", "parameter_two", "parameter_three"]
        .iter()
        .zip(args)
    {
        result["fields"][*slot] = value.clone()
    }
    Ok(result)
}
fn html_encode(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if !c.is_ascii() => out.push_str(&format!("&#x{:x};", c as u32)),
            _ => out.push(c),
        }
    }
    out
}
fn entity_literal(text: &str) -> bool {
    let Some(body) = text.strip_prefix('&').and_then(|s| s.strip_suffix(';')) else {
        return false;
    };
    if let Some(n) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        !n.is_empty() && n.chars().all(|c| c.is_ascii_hexdigit())
    } else if let Some(n) = body.strip_prefix('#') {
        !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
    } else {
        let mut chars = body.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric())
            && body.len() > 1
    }
}
fn symbol(value: &Value, normalize: bool) -> Result<Value, Value> {
    if !value.is_string() {
        return Ok(value.clone());
    }
    let raw = value.as_str().unwrap();
    let encoded;
    let name = if normalize && !entity_literal(raw) {
        encoded = html_encode(raw);
        encoded.as_str()
    } else {
        raw
    };
    let found = &declarations()["symbols"][name.trim()];
    if core(found) {
        Ok(found.clone())
    } else {
        Ok(node("Math::Symbols::Symbol", json!({"value":name})))
    }
}
fn text(v: &Value) -> Value {
    node(
        "Math::Function::Text",
        json!({"lang":null,"parameter_one":v}),
    )
}
fn row(v: &Value) -> Value {
    node("Math::Function::Tr", json!({"parameter_one":list(v)}))
}
fn cell(v: &Value) -> Value {
    let values: Vec<_> = list(v).into_iter().filter(|x| x != "&").collect();
    node(
        "Math::Function::Td",
        json!({"parameter_one":values,"parameter_two":null}),
    )
}
fn table(v: &Value) -> Value {
    node(
        "Math::Function::Table",
        json!({"close_paren":null,"open_paren":null,"options":{},"value":list(v)}),
    )
}
fn linebreak() -> Value {
    node(
        "Math::Function::Linebreak",
        json!({"attributes":{},"parameter_one":null}),
    )
}
fn atomic(key: &str, value: &Value) -> Result<Value, Value> {
    Ok(match key {
        "text" => text(value),
        "number" => number(
            value
                .as_str()
                .ok_or_else(|| error("source HTML number must be text"))?,
        ),
        "symbol" => symbol(value, true)?,
        "unary" => function(
            value
                .as_str()
                .ok_or_else(|| error("source HTML unary must be text"))?,
            &[],
        )?,
        "sum_prod" => {
            let name = declarations()["sub_sup"][value
                .as_str()
                .ok_or_else(|| error("source HTML sum/prod must be text"))?]
            .as_str()
            .ok_or_else(|| error("source HTML sum/prod name absent"))?;
            function(name, &[])?
        }
        "linebreak" => linebreak(),
        "tr_value" => row(value),
        "td_value" => cell(value),
        "table_value" => table(value),
        "expression" | "sequence" | "parse_parenthesis" | "unary_function" => value.clone(),
        _ => {
            return Err(error(format!(
                "unimplemented source HTML atomic rule {key}"
            )))
        }
    })
}
fn sub_sup(object: &Map<String, Value>) -> Value {
    let base = object["sub_sup"].clone();
    let sub = object.get("sub_value").map(filter).unwrap_or(Value::Null);
    let sup = object.get("sup_value").map(filter).unwrap_or(Value::Null);
    let mut result = if [
        "Math::Function::Prod",
        "Math::Function::Sum",
        "Math::Function::Log",
        "Math::Function::Lim",
    ]
    .contains(&base["class"].as_str().unwrap_or(""))
    {
        let mut v = base;
        if !sub.is_null() {
            v["fields"]["parameter_one"] = sub
        }
        if !sup.is_null() {
            v["fields"]["parameter_two"] = sup
        }
        v
    } else if !sub.is_null() && !sup.is_null() {
        node(
            "Math::Function::PowerBase",
            json!({"parameter_one":base,"parameter_two":sub,"parameter_three":sup}),
        )
    } else if !sup.is_null() {
        node(
            "Math::Function::Power",
            json!({"parameter_one":base,"parameter_two":sup}),
        )
    } else {
        node(
            "Math::Function::Base",
            json!({"parameter_one":base,"parameter_two":sub}),
        )
    };
    if let Some(tail) = object.get("expression") {
        result = append(result, tail)
    }
    result
}
fn radix_decimal(input: &str, radix: u32) -> String {
    // Ruby String#to_i(radix) keeps arbitrary precision and stops at the first
    // non-digit. Decimal digits avoid a target-machine integer truncation.
    let mut out = vec![0u32];
    for c in input.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        let mut carry = d;
        for digit in out.iter_mut().rev() {
            let x = *digit * radix + carry;
            *digit = x % 10;
            carry = x / 10
        }
        while carry > 0 {
            out.insert(0, carry % 10);
            carry /= 10
        }
    }
    out.into_iter()
        .map(|x| char::from_digit(x, 10).unwrap())
        .collect()
}
fn action(object: &Map<String, Value>) -> Result<Value, Value> {
    if object.contains_key("sub_sup") {
        return Ok(sub_sup(object));
    }
    if object.len() == 1 {
        let (key, value) = object.iter().next().unwrap();
        if ["hex_number", "binary_number", "octal_number"].contains(&key.as_str()) {
            let raw = value
                .as_str()
                .ok_or_else(|| error("source base-number capture must be text"))?;
            let radix = if key == "hex_number" {
                16
            } else if key == "binary_number" {
                2
            } else {
                8
            };
            let mut result = number(if radix == 16 { raw } else { "" });
            if radix != 16 {
                result["fields"]["value"] = Value::String(radix_decimal(raw, radix))
            }
            result["fields"]["base"] = json!(radix);
            return Ok(result);
        }
        return atomic(key, value);
    }
    if let Some(binary) = object.get("binary") {
        let mut first = object["first_value"].clone();
        let mut second = object["second_value"].clone();
        if first.is_array() && second.is_array() {
            first = filter(&first);
            second = filter(&second)
        }
        return function(
            binary
                .as_str()
                .ok_or_else(|| error("source binary name must be text"))?,
            &[first, second],
        );
    }
    if let Some(unary) = object.get("unary") {
        return function(
            unary
                .as_str()
                .ok_or_else(|| error("source unary name must be text"))?,
            &[object["first_value"].clone()],
        );
    }
    if object.contains_key("lparen") {
        let mut content = vec![symbol(&object["lparen"], false)?];
        for key in ["text", "number", "sequence", "unary_function"] {
            if let Some(value) = object.get(key) {
                content.push(if key == "sequence" && value.is_array() {
                    filter(value)
                } else {
                    atomic(key, value)?
                });
                break;
            }
        }
        if let Some(value) = object.get("parse_parenthesis") {
            content.push(value.clone())
        }
        if let Some(value) = object.get("expression") {
            content.extend(list(value))
        }
        content.push(symbol(&object["rparen"], false)?);
        return Ok(formula(content));
    }
    if object.contains_key("unary_function") && object.contains_key("sequence") {
        return Ok(formula(list(&append(
            object["unary_function"].clone(),
            &object["sequence"],
        ))));
    }
    for key in [
        "linebreak",
        "tr_value",
        "td_value",
        "text",
        "symbol",
        "number",
        "sequence",
    ] {
        if let Some(value) = object.get(key) {
            let first = atomic(key, value)?;
            if let Some(tail) = object.get("expression") {
                if key == "sequence" && first.is_array() {
                    let mut values = list(&first);
                    values.extend(list(tail));
                    return Ok(Value::Array(values));
                }
                return Ok(append(first, tail));
            }
            if let Some(tail) = object.get("parse_parenthesis") {
                let values = vec![first, tail.clone()];
                return Ok(if key == "text" {
                    formula(values)
                } else {
                    Value::Array(values)
                });
            }
        }
    }
    Err(error("unimplemented matched source HTML transform rule"))
}
fn apply(value: &Value) -> Result<Value, Value> {
    match value {
        Value::Array(values) => values
            .iter()
            .map(apply)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(object) => {
            let transformed: Map<String, Value> = object
                .iter()
                .map(|(k, v)| apply(v).map(|v| (k.clone(), v)))
                .collect::<Result<_, _>>()?;
            for rule in declarations()["rules"].as_array().unwrap() {
                if matches(rule, &transformed) {
                    return action(&transformed);
                }
            }
            // Source Parsanol::Transform is non-strict: unmatched Hash nodes are
            // retained, including mixed Hash/Array values, without guessing.
            Ok(Value::Object(transformed))
        }
        _ => Ok(value.clone()),
    }
}
/// Html::Parser#parse result; wrapper assigns original input_string afterward.
pub(super) fn transform(tree: &Value, _input: &str) -> Result<Value, Value> {
    let result = apply(tree)?;
    Ok(if result["class"] == "Math::Formula" {
        result
    } else {
        formula(list(&result))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_source_scope_and_base_prefix_captures_keep_model_semantics() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../fixtures/html-parse/source-scope-cases.json"
        ))
        .unwrap();
        let mut accepted = 0;
        for row in fixture["rows"].as_array().unwrap() {
            if row.get("model").is_none() {
                continue;
            }
            assert_eq!(
                transform(&row["captures"], row["input"].as_str().unwrap()).unwrap(),
                row["model"],
                "source HTML scope {}",
                row["id"]
            );
            accepted += 1;
        }
        assert_eq!(accepted, 19);
    }
    #[test]
    fn original_ruby_html_captures_preserve_all_common_model_shapes() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../fixtures/html-parse/source-models.json"))
                .unwrap();
        assert_eq!(fixture["rows"].as_array().unwrap().len(), 525);
        for row in fixture["rows"].as_array().unwrap() {
            assert_eq!(
                transform(&row["captures"], row["input"].as_str().unwrap()).unwrap(),
                row["model"],
                "source HTML model {}",
                row["id"]
            );
        }
    }
}
