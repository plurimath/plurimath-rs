//! Source LaTeX table organization over accepted capture nodes, without a grammar.
use super::{incomplete, tx};
use crate::model::{formula, node};
use serde_json::{json, Map, Value};
enum Item {
    Node(Value),
    Column,
    Row,
}
fn items(value: &Value, depth: usize, output: &mut Vec<Item>) -> Result<(), Value> {
    if let Some(object) = value.as_object() {
        if object.len() == 2 && object.contains_key("sequence") && object.contains_key("expression")
        {
            items(&object["sequence"], depth + 1, output)?;
            items(&object["expression"], depth + 1, output)?;
            return Ok(());
        }
        if object.len() == 1 && object.contains_key("\\\\") {
            output.push(Item::Row);
            return Ok(());
        }
    }
    let separator = crate::symbols::symbol_for("&", "latex");
    for value in tx("latex", value, depth + 1)? {
        if value["class"] == separator["class"] {
            output.push(Item::Column)
        } else if value["class"] == "Math::Symbols::Hline" {
            return Err(incomplete("unported table horizontal-line options"));
        } else {
            output.push(Item::Node(value))
        }
    }
    Ok(())
}
fn cell(mut values: Vec<Value>, filter: bool, align: Option<&str>) -> Value {
    if filter {
        let mut index = 0;
        while index < values.len() {
            if values[index]["class"] == "Math::Symbols::Minus" {
                let mut pair = vec![values[index].clone()];
                if index + 1 < values.len() {
                    pair.push(values.remove(index + 1));
                } else { pair.push(Value::Null); }
                values[index] = formula(pair);
            }
            index += 1;
        }
    }
    node(
        "Math::Function::Td",
        json!({"parameter_one":values,"parameter_two":align.map(|value|json!({"columnalign":value}))}),
    )
}
pub(super) fn parse(m: &Map<String, Value>, depth: usize) -> Result<Vec<Value>, Value> {
    if m.keys()
        .any(|key| !["environment", "ending", "table_data", "args"].contains(&key.as_str()))
    {
        return Err(incomplete("unported table environment capture fields"));
    }
    let name = m["environment"]
        .as_str()
        .ok_or_else(|| incomplete("non-string table environment"))?;
    let (class, left, right) = match name {
        "matrix" => ("Matrix", None, None),
        "array" => ("Array", None, None),
        "pmatrix" => ("Pmatrix", Some("Lround"), Some("Rround")),
        "bmatrix" => ("Bmatrix", Some("Lsquare"), Some("Rsquare")),
        "Bmatrix" => ("Bmatrix", Some("Lcurly"), Some("Rcurly")),
        "vmatrix" => ("Vmatrix", Some("Vert"), Some("Vert")),
        _ => {
            return Err(incomplete(format!(
                "unported source table environment {name}"
            )))
        }
    };
    let mut alignments = Vec::new();
    if let Some(args) = m.get("args") {
        for value in tx("latex", args, depth + 1)? {
            alignments.push(match value["fields"]["value"].as_str() {
                Some("c") => "center",
                Some("l") => "left",
                Some("r") => "right",
                _ => return Err(incomplete("unported table alignment or separator option")),
            });
        }
    }
    let body = m
        .get("table_data")
        .ok_or_else(|| incomplete("missing source table data"))?;
    let mut tokens = Vec::new();
    items(body, depth + 1, &mut tokens)?;
    let mut rows = Vec::new();
    let mut cells = Vec::new();
    let mut values = Vec::new();
    for token in tokens {
        match token {
            Item::Node(value) => values.push(value),
            Item::Column => {
                cells.push(cell(
                    std::mem::take(&mut values),
                    true,
                    alignments.get(cells.len()).copied(),
                ));
            }
            Item::Row => {
                cells.push(cell(
                    std::mem::take(&mut values),
                    true,
                    alignments.get(cells.len()).copied(),
                ));
                rows.push(node(
                    "Math::Function::Tr",
                    json!({"parameter_one":std::mem::take(&mut cells)}),
                ));
            }
        }
    }
    if !(values.is_empty() && cells.is_empty() && !rows.is_empty()) {
        cells.push(cell(values, false, alignments.get(cells.len()).copied()));
    }
    if !cells.is_empty() {
        rows.push(node("Math::Function::Tr", json!({"parameter_one":cells})));
    }
    let delimiter = |name: Option<&str>| {
        name.map_or(Value::Null, |id| {
            node(
                &format!("Math::Symbols::Paren::{id}"),
                json!({"value":null}),
            )
        })
    };
    Ok(vec![node(
        &format!("Math::Function::Table::{class}"),
        json!({"value":rows,"options":{},"open_paren":delimiter(left),"close_paren":delimiter(right)}),
    )])
}
