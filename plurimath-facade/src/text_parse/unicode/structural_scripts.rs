//! UnicodeMath structural script rules, pinned to Ruby 0a1df68.
use super::{keys, simple, unf, unfenced, Rec};
use crate::model::{formula, node};
use serde_json::{json, Map, Value};

fn base(first: Value, second: Value) -> Value {
    // Base's empty options hash is absent in the source object.
    node(
        "Math::Function::Base",
        json!({"parameter_one":first,"parameter_two":second}),
    )
}

/// Utility.recursive_sub/base_recursion insert at the deepest Base base.
/// Fenced arguments remain fenced: recursive_sub never calls unfenced_value.
fn recursive_sub(first: Value, mut rest: Value) -> Value {
    let mut cursor = &mut rest;
    while cursor["class"] == "Math::Function::Base" {
        cursor = &mut cursor["fields"]["parameter_one"];
    }
    *cursor = base(first, cursor.take());
    rest
}

fn power(first: Value, second: Value) -> Value {
    node(
        "Math::Function::Power",
        json!({"parameter_one":first,"parameter_two":second}),
    )
}

/// Utility.recursive_sup/sup_recursion insert at the deepest Power base
/// (rules.rb 1007-1040). A non-Power recursion becomes the exponent.
fn recursive_sup(first: Value, mut rest: Value) -> Value {
    if rest["class"] != "Math::Function::Power" {
        return power(first, rest);
    }
    let mut cursor = &mut rest["fields"]["parameter_one"];
    while cursor["class"] == "Math::Function::Power" {
        cursor = &mut cursor["fields"]["parameter_one"];
    }
    *cursor = power(first, cursor.take());
    rest
}

fn empty_binary(value: &Value) -> bool {
    ["Obrace", "Oint", "Prod", "Sum", "Int", "Lim", "Inf", "Log"]
        .iter()
        .any(|kind| value["class"] == format!("Math::Function::{kind}"))
        && (value["fields"]["parameter_one"].is_null() || value["fields"]["parameter_one"] == false)
}

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if keys(m, &["under", "sub_script"]) {
        let script = simple(rec(&m["sub_script"])?, "under script")?;
        return Ok(Some(vec![node(
            "Math::Function::Underset",
            json!({"options":{},"parameter_one":unfenced(script),"parameter_two":null}),
        )]));
    }
    if keys(m, &["sub_script", "sub_recursion"]) {
        let first = simple(rec(&m["sub_script"])?, "recursive subscript")?;
        let rest = simple(rec(&m["sub_recursion"])?, "subscript recursion")?;
        return Ok(Some(vec![recursive_sub(first, rest)]));
    }
    if keys(m, &["sup_script", "sup_recursion"]) {
        let script = rec(&m["sup_script"])?;
        let first = if script.len() == 1 && !m["sup_script"].is_array() {
            script.into_iter().next().unwrap()
        } else {
            formula(script)
        };
        let rest = unfenced(simple(rec(&m["sup_recursion"])?, "superscript recursion")?);
        return Ok(Some(vec![recursive_sup(first, rest)]));
    }
    // rules.rb 644-657: an operator subscript recursion.
    if keys(m, &["operator", "sub_recursion"]) {
        let operator = m["operator"]
            .as_str()
            .map(super::symbol)
            .ok_or_else(|| super::super::incomplete("non-text operator capture"))?;
        let mut rest = rec(&m["sub_recursion"])?;
        if rest.len() == 1 && !m["sub_recursion"].is_array() {
            return Ok(Some(vec![recursive_sub(operator, rest.pop().unwrap())]));
        }
        rest.insert(0, operator);
        return Ok(Some(rest));
    }
    // Source list rules preserve the mini-sized fields in each built node.
    // In particular, recursive numerators become a list before fractions
    // filter it into a Formula; they never attach to the superscript itself.
    if keys(m, &["mini_sup", "recursive_numerator"]) {
        let mut values = vec![simple(rec(&m["mini_sup"])?, "mini_sup")?];
        values.extend(rec(&m["recursive_numerator"])?);
        return Ok(Some(values));
    }
    if keys(m, &["sub_script", "mini_sub"]) {
        return Ok(Some(vec![
            simple(rec(&m["sub_script"])?, "sub_script")?,
            simple(rec(&m["mini_sub"])?, "mini_sub")?,
        ]));
    }
    for tails in [["mini_sup", "operand"], ["mini_sub", "exp_iteration"]] {
        if keys(m, &["sub_script", tails[0], tails[1]]) {
            return Ok(Some(vec![
                simple(rec(&m["sub_script"])?, "subscript")?,
                simple(rec(&m[tails[0]])?, tails[0])?,
                simple(rec(&m[tails[1]])?, tails[1])?,
            ]));
        }
    }

    // Complete the source base/sub rules for an under-script placeholder.
    // This is deliberately gated to that capture, leaving other base/script
    // dispatch with its existing owner. Source rules.rb 1041 and 2125.
    if (keys(m, &["base", "sub"]) || keys(m, &["base", "sub", "sup"]))
        && m["sub"]
            .as_object()
            .is_some_and(|sub| keys(sub, &["under", "sub_script"]))
    {
        let mut bases = rec(&m["base"])?;
        let single = bases.len() == 1 && !m["base"].is_array();
        let mut first = bases.pop().unwrap_or(Value::Null);
        let mut under = simple(rec(&m["sub"])?, "under")?;
        let built = if m.contains_key("sup") {
            let over_values = rec(&m["sup"])?;
            if single
                && over_values.len() == 1
                && matches!(
                    over_values[0]["class"].as_str(),
                    Some("Math::Function::Overset" | "Math::Function::Underset")
                )
            {
                node(
                    "Math::Function::Underover",
                    json!({
                        "parameter_one":first,
                        "parameter_two":under["fields"]["parameter_one"],
                        "parameter_three":over_values[0]["fields"]["parameter_one"]
                    }),
                )
            } else {
                node(
                    "Math::Function::PowerBase",
                    json!({
                        "parameter_one":first,"parameter_two":under,
                        "parameter_three":unf(over_values)
                    }),
                )
            }
        } else if single && empty_binary(&first) {
            first["fields"]["parameter_one"] = under["fields"]["parameter_one"].clone();
            first
        } else {
            under["fields"]["parameter_two"] = first;
            under
        };
        bases.push(built);
        return Ok(Some(bases));
    }
    Ok(None)
}
