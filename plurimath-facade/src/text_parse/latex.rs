//! Source LaTeX mapper builders (plurimath 0a1df68 `Latex::Mapper::Build`,
//! `Latex::Utility`) and source constructor invariants over common nodes.
use super::{incomplete, tx};
use crate::model::{formula, node};
use serde_json::{json, Map, Value};

/// Symbol classes whose source `is_nary_symbol?` is true (plurimath 0a1df68 math/symbols).
const NARY_SYMBOLS: [&str; 13] = [
    "Clockoint",
    "Cntclockoint",
    "Coprod",
    "Bigwedge",
    "Dint",
    "Duni",
    "Iiiint",
    "Iiint",
    "Iint",
    "Intclockwise",
    "Oiiint",
    "Oiint",
    "Oint",
];

fn nary_symbol(value: &Value) -> bool {
    value["class"]
        .as_str()
        .and_then(|class| class.strip_prefix("Math::Symbols::"))
        .is_some_and(|name| NARY_SYMBOLS.contains(&name))
}

/// Source `Utility.filter_values` over a converted value list.
pub(super) fn filter_values(mut values: Vec<Value>) -> Value {
    match values.len() {
        0 => Value::Null,
        1 => values.pop().unwrap(),
        _ => formula(values),
    }
}

/// Source `Build.nary`: a PowerBase over an n-ary symbol followed by its operand
/// becomes one Nary; any other sequence keeps its values in order.
pub(super) fn nary(sequence: Vec<Value>, expression: Vec<Value>) -> Vec<Value> {
    if let [base] = sequence.as_slice() {
        if base["class"] == "Math::Function::PowerBase"
            && nary_symbol(&base["fields"]["parameter_one"])
        {
            let fields = &base["fields"];
            return vec![node(
                "Math::Function::Nary",
                json!({"options":{},"parameter_one":fields["parameter_one"],"parameter_two":fields["parameter_two"],"parameter_three":fields["parameter_three"],"parameter_four":filter_values(expression)}),
            )];
        }
    }
    let mut values = sequence;
    values.extend(expression);
    values
}

pub(super) fn linebreak() -> Value {
    node(
        "Math::Function::Linebreak",
        json!({"attributes":{},"parameter_one":null}),
    )
}

fn separates(value: &Value) -> bool {
    value["class"] == "Math::Function::Linebreak"
        || value["class"] == "Math::Symbols::Ampersand"
        || (value["class"] == "Math::Symbols::Symbol"
            && (value["fields"]["value"] == "&" || value["fields"]["value"] == "\\\\"))
}

fn breaks_row(value: &Value) -> bool {
    value["class"] == "Math::Function::Linebreak"
        || (value["class"] == "Math::Symbols::Symbol" && value["fields"]["value"] == "\\\\")
}

fn td(values: Vec<Value>) -> Value {
    node(
        "Math::Function::Td",
        json!({"parameter_one":values,"parameter_two":null}),
    )
}

/// Source `Latex::Utility.filter_table_data`: a Minus joins the value after it.
fn filter_table_data(mut values: Vec<Value>) -> Vec<Value> {
    let mut index = 0;
    while index < values.len() {
        if values[index]["class"] == "Math::Symbols::Minus" {
            let next = if index + 1 < values.len() {
                values.remove(index + 1)
            } else {
                Value::Null
            };
            values[index] = formula(vec![values[index].clone(), next]);
        }
        index += 1;
    }
    values
}

/// Source `Latex::Utility.organize_table` without column alignment.
pub(super) fn organize_table(values: Vec<Value>) -> Vec<Value> {
    let mut table = Vec::new();
    let mut data = Vec::new();
    let mut row = Vec::new();
    for value in values {
        if separates(&value) {
            row.push(td(filter_table_data(std::mem::take(&mut data))));
            if breaks_row(&value) {
                table.push(node(
                    "Math::Function::Tr",
                    json!({"parameter_one":std::mem::take(&mut row)}),
                ));
            }
            continue;
        }
        data.push(value);
    }
    if !(data.is_empty() && row.is_empty() && !table.is_empty()) {
        row.push(td(data));
    }
    if !row.is_empty() {
        table.push(node("Math::Function::Tr", json!({"parameter_one":row})));
    }
    table
}

/// Source `ModelHelper.validate_left_right`, run by every function constructor
/// except tables: a direct Formula operand that starts with Left keeps its wrapper.
pub(super) fn validate_left_right(value: &mut Value) {
    if let Some(items) = value.as_array_mut() {
        items.iter_mut().for_each(validate_left_right);
        return;
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let class = object
        .get("class")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(fields) = object.get_mut("fields").and_then(Value::as_object_mut) {
        let validates = class.starts_with("Math::Function::")
            && !class.starts_with("Math::Function::Table")
            && class != "Math::Function::Unitsml";
        for (key, field) in fields.iter_mut() {
            validate_left_right(field);
            if validates
                && key.starts_with("parameter_")
                && field["class"] == "Math::Formula"
                && field["fields"]["value"][0]["class"] == "Math::Function::Left"
            {
                field["fields"]["left_right_wrapper"] = json!(true);
            }
        }
    }
}

/// Source LaTeX captures with a dedicated `Latex::Mapper` entry that the shared
/// converter has no shape for. `None` leaves the capture to the shared converter.
pub(super) fn capture(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, Value> {
    let keys = {
        let mut keys: Vec<&str> = m.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys
    };
    // An absent capture (nil, or the `{}` absent wrapper) is nil in the source
    // constructor, never an empty Formula.
    let value = |key: &str| -> Result<Value, Value> {
        let values = tx("latex", &m[key], depth + 1)?;
        Ok(if values.is_empty() {
            Value::Null
        } else {
            super::one(values)
        })
    };
    match keys.as_slice() {
        // RENAMES row_break => "\\\\"; SINGLE row_break builds a Linebreak.
        ["\\\\"] => Ok(Some(vec![linebreak()])),
        ["expression", "substack"] => {
            let values = tx("latex", &m["expression"], depth + 1)?;
            if values.len() < 2 {
                // TYPED requires a sequence; a single value stays an unbuilt capture hash.
                return Err(incomplete("unported single-value substack capture"));
            }
            Ok(Some(vec![node(
                "Math::Function::Substack",
                json!({"parameter_one":organize_table(values)}),
            )]))
        }
        ["base", "first_value", "power"] => Ok(Some(vec![node(
            "Math::Function::Limits",
            json!({"parameter_one":value("first_value")?,"parameter_two":value("base")?,"parameter_three":value("power")?}),
        )])),
        ["first_value", "rule", "second_value", "third_value"] => Ok(Some(vec![node(
            "Math::Function::Rule",
            json!({"parameter_one":value("first_value")?,"parameter_two":value("second_value")?,"parameter_three":value("third_value")?}),
        )])),
        // TYPED {power, expression|symbols|number} => Power and {base, expression} => Base,
        // reached through the `over` wrapper; the scripted operand is kept as built.
        ["expression", "power"] | ["number", "power"] | ["power", "symbols"] => {
            let script = keys.iter().find(|key| **key != "power").unwrap();
            let mut operand = Map::new();
            operand.insert((*script).to_string(), m[*script].clone());
            Ok(Some(vec![node(
                "Math::Function::Power",
                json!({"parameter_one":value("power")?,"parameter_two":super::one(tx("latex", &Value::Object(operand), depth + 1)?)}),
            )]))
        }
        ["base", "expression"] => Ok(Some(vec![node(
            "Math::Function::Base",
            json!({"parameter_one":value("base")?,"parameter_two":value("expression")?}),
        )])),
        _ => Ok(None),
    }
}

/// Source `Utility.html_entity_to_unicode` for number text: the text frontends'
/// preprocessing encodes non-ASCII as hexadecimal references, which the source
/// Number constructor input decodes. Only numeric references can occur here.
pub(super) fn number_text(text: &str) -> String {
    if !text.contains("&#") {
        return text.to_string();
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("&#") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let decoded = tail.find(';').and_then(|end| {
            let body = &tail[..end];
            let code = match body.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => body.parse::<u32>().ok(),
            }?;
            char::from_u32(code).map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push_str("&#");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Source LaTeX preprocessing decodes numeric character references
/// (`HTMLEntities#decode`); a reference to a surrogate or to a value above
/// U+10FFFF cannot be decoded and Math.parse raises ParseError.
pub(super) fn undecodable_reference(input: &str) -> bool {
    let mut rest = input;
    while let Some(start) = rest.find("&#") {
        let tail = &rest[start + 2..];
        let (radix, digits) = match tail.strip_prefix(['x', 'X']) {
            Some(hex) => (16, hex),
            None => (10, tail),
        };
        let end = digits
            .find(|c: char| !c.is_digit(radix))
            .unwrap_or(digits.len());
        if end > 0 && digits[end..].starts_with(';') {
            let code = u32::from_str_radix(&digits[..end], radix).unwrap_or(u32::MAX);
            if char::from_u32(code).is_none() {
                return true;
            }
        }
        rest = tail;
    }
    false
}
