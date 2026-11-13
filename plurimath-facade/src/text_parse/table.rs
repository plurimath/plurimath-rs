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
        } else {
            output.push(Item::Node(value))
        }
    }
    Ok(())
}
fn cell(mut values: Vec<Value>, filter: bool) -> Value {
    if filter {
        let mut index = 0;
        while index < values.len() {
            if values[index]["class"] == "Math::Symbols::Minus" {
                let mut pair = vec![values[index].clone()];
                if index + 1 < values.len() {
                    pair.push(values.remove(index + 1));
                } else {
                    pair.push(Value::Null);
                }
                values[index] = formula(pair);
            }
            index += 1;
        }
    }
    node(
        "Math::Function::Td",
        json!({"parameter_one":values,"parameter_two":null}),
    )
}
/// `Constants::MATRICES` opening delimiter, `MATRICES_PARENTHESIS` closing one and
/// `Utility.get_table_class` class for each source environment (`cases` is special
/// cased in `Build.table`).
fn environment(name: &str) -> Option<(&'static str, Option<&'static str>, Option<&'static str>)> {
    Some(match name {
        "matrix" => ("Matrix", None, None),
        "array" => ("Array", None, None),
        "align" => ("Align", None, None),
        "split" => ("Split", None, None),
        "multline" => ("Multline", None, None),
        "pmatrix" => ("Pmatrix", Some("("), Some(")")),
        "bmatrix" => ("Bmatrix", Some("["), Some("]")),
        "Bmatrix" => ("Bmatrix", Some("{"), Some("}")),
        "vmatrix" => ("Vmatrix", Some("|"), Some("|")),
        "Vmatrix" => ("Vmatrix", Some("\\Vert"), Some("]")),
        "cases" => ("Cases", Some("{"), Some(":}")),
        _ => return None,
    })
}
/// `Utility.symbols_class(paren, lang: :latex, table: true)`.
fn delimiter(text: Option<&str>) -> Value {
    match text {
        None => Value::Null,
        Some("{") => node("Math::Symbols::Paren::Lcurly", json!({"value":null})),
        Some("}") => node("Math::Symbols::Paren::Rcurly", json!({"value":null})),
        Some(text) => crate::symbols::symbol_for(text, "latex"),
    }
}
/// A column specifier as `organize_table`'s `string_columns` entry.
fn column(value: &Value) -> Result<Option<String>, Value> {
    let class = value["class"].as_str().unwrap_or("");
    if class.starts_with("Math::Symbols::Paren::") {
        return Ok(Some("|".into()));
    }
    match &value["fields"]["value"] {
        Value::String(text) => Ok(Some(text.clone())),
        Value::Null => Ok(None),
        _ => Err(incomplete("unported table column specifier")),
    }
}
fn alignment(letter: Option<&String>) -> Option<&'static str> {
    match letter.map(String::as_str) {
        Some("c") => Some("center"),
        Some("r") => Some("right"),
        Some("l") => Some("left"),
        _ => None,
    }
}
pub(super) fn parse(m: &Map<String, Value>, depth: usize) -> Result<Vec<Value>, Value> {
    if m.keys().any(|key| {
        ![
            "environment",
            "ending",
            "table_data",
            "args",
            "asterisk",
            "options",
            "expression",
        ]
        .contains(&key.as_str())
    }) || (m.contains_key("options") && !m.contains_key("asterisk"))
        // `\matrix{...}`: TYPED `{ environment:, expression: }` with no other field.
        || (m.contains_key("expression") && m.len() != 2)
        || (m.contains_key("asterisk") && m.contains_key("args"))
    {
        return Err(incomplete("unported table environment capture fields"));
    }
    let name = m["environment"]
        .as_str()
        .ok_or_else(|| incomplete("non-string table environment"))?;
    let (class, open, close) = environment(name)
        .ok_or_else(|| incomplete(format!("unported source table environment {name}")))?;
    let asterisk = m.contains_key("asterisk");
    // Build.table's column_align: `args` (TYPED args entries) or the asterisk `options`.
    let specifier = m.get("args").or_else(|| m.get("options"));
    let mut columns = Vec::new();
    if let Some(value) = specifier {
        for value in tx("latex", value, depth + 1)? {
            columns.push(column(&value)?);
        }
    }
    if asterisk && columns.len() > 1 {
        // TYPED matches the asterisk `options` only as one simple value.
        return Err(incomplete("unported table alignment or separator option"));
    }
    let body = m
        .get("table_data")
        .or_else(|| m.get("expression"))
        .ok_or_else(|| incomplete("missing source table data"))?;
    let mut tokens = Vec::new();
    items(body, depth + 1, &mut tokens)?;
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut cells = Vec::new();
    let mut values = Vec::new();
    for token in tokens {
        match token {
            Item::Node(value) => values.push(value),
            Item::Column => cells.push(cell(std::mem::take(&mut values), true)),
            Item::Row => {
                cells.push(cell(std::mem::take(&mut values), true));
                rows.push(std::mem::take(&mut cells));
            }
        }
    }
    if !(values.is_empty() && cells.is_empty() && !rows.is_empty()) {
        cells.push(cell(values, false));
    }
    if !cells.is_empty() {
        rows.push(cells);
    }
    // organize_tds: `|` entries are dropped; the asterisk `options` form repeats the
    // remaining letters once per cell of the row.
    let letters: Vec<Option<String>> = columns
        .iter()
        .filter(|c| c.as_deref() != Some("|"))
        .cloned()
        .collect();
    for row in rows.iter_mut() {
        let letters: Vec<Option<String>> = if asterisk {
            (0..row.len())
                .flat_map(|_| letters.iter().cloned())
                .collect()
        } else {
            letters.clone()
        };
        for (index, td) in row.iter_mut().enumerate() {
            if let Some(align) = alignment(letters.get(index).and_then(Option::as_ref)) {
                td["fields"]["parameter_two"] = json!({"columnalign":align});
            }
        }
    }
    // Utility.table_separator(string_columns, table, symbol: "|").
    let mut rows: Vec<Vec<Option<Value>>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(Some).collect())
        .collect();
    for (index, separator) in columns.iter().enumerate() {
        if separator.as_deref() != Some("|") {
            continue;
        }
        for row in rows.iter_mut() {
            let rule = node(
                "Math::Function::Td",
                json!({"parameter_one":[node("Math::Symbols::Paren::Vert",json!({"value":null}))],"parameter_two":null}),
            );
            while row.len() < index {
                row.push(None);
            }
            row.insert(index, Some(rule));
            if let Some(slot) = row.iter_mut().find(|td| td.is_none()) {
                *slot = Some(node(
                    "Math::Function::Td",
                    json!({"parameter_one":[],"parameter_two":null}),
                ));
            }
        }
    }
    if rows.iter().flatten().any(Option::is_none) {
        return Err(incomplete("unported sparse table separator row"));
    }
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            node(
                "Math::Function::Tr",
                json!({"parameter_one":row.into_iter().map(Option::unwrap).collect::<Vec<_>>()}),
            )
        })
        .collect();
    let options = if asterisk {
        json!({"asterisk":true})
    } else if m.contains_key("args") {
        // Utility.table_options: a rowline when any row opens with an Hline.
        let lines: Vec<&str> = rows
            .iter()
            .map(|row| {
                if row["fields"]["parameter_one"][0]["fields"]["parameter_one"][0]["class"]
                    == "Math::Symbols::Hline"
                {
                    "solid"
                } else {
                    "none"
                }
            })
            .collect();
        if lines.contains(&"solid") {
            json!({"rowline":lines.join(" ")})
        } else {
            json!({})
        }
    } else {
        json!({})
    };
    Ok(vec![node(
        &format!("Math::Function::Table::{class}"),
        json!({"value":rows,"options":options,"open_paren":delimiter(open),"close_paren":delimiter(close)}),
    )])
}
