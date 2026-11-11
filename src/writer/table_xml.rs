//! Source Table/Tr/Td XML methods, preserving row and cell insertion boundaries.
use super::{attrs, control_properties, el, render_xml, text, Model, Value, Xml};

fn values<'a>(value: &'a Value, role: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("unported source {role} value, expected a node array"))
}
fn model(value: &Value) -> Result<Model, String> {
    Model::from_value(value)
}
fn vertical_bar(value: &Value) -> bool {
    value["class"] == "Math::Symbols::Paren::Vert"
        || (value["class"] == "Math::Symbols::Symbol"
            && matches!(value["fields"]["value"].as_str(), Some("|" | "&#x7c;")))
}
fn attribute_values(value: &Value) -> Result<Vec<(String, String)>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let object = value
        .as_object()
        .ok_or("unported non-hash source table attributes")?;
    object
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(name, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Bool(_) | Value::Number(_) => value.to_string(),
                _ => return Err(format!("unported structured table attribute {name}")),
            };
            let text = crate::xml_parse::source_entity_decode(&text)
                .map_err(|_| format!("unported source table attribute entity {name}"))?;
            Ok((name.clone(), text))
        })
        .collect()
}
fn set_attribute(attributes: &mut Vec<(String, String)>, name: &str, value: String) {
    if let Some((_, current)) = attributes.iter_mut().find(|(key, _)| key == name) {
        *current = value;
    } else {
        attributes.push((name.into(), value));
    }
}
fn table_attributes(table: &Model, rows: &[Value]) -> Result<Vec<(String, String)>, String> {
    let first = model(
        rows.first()
            .ok_or("unported source empty table attribute access")?,
    )?;
    let mut columns = Vec::<String>::new();
    for (index, cell) in values(first.field("parameter_one"), "table row")?
        .iter()
        .enumerate()
    {
        let cell = model(cell)?;
        let bar = values(cell.field("parameter_one"), "table cell")?
            .iter()
            .any(vertical_bar);
        if bar {
            if columns.is_empty() {
                columns.push("solid".into());
            } else {
                let offset = index
                    .checked_sub(1)
                    .ok_or("source column separator index")?;
                columns.resize(columns.len().max(offset + 1), String::new());
                columns[offset] = "solid".into();
            }
        } else {
            columns.push("none".into());
        }
    }
    let mut attributes = attribute_values(table.field("options"))?;
    attributes.retain(|(name, _)| name != "asterisk");
    if columns.iter().any(|value| value == "solid") {
        set_attribute(&mut attributes, "columnlines", columns.join(" "));
    }
    if table.field("close_paren")["class"] == "Math::Symbols::Paren::CloseParen" {
        set_attribute(&mut attributes, "columnalign", "left".into());
    }
    Ok(attributes)
}
fn parenthesis(value: &Value, format: &str) -> Result<String, String> {
    if value.is_null() {
        return Ok(String::new());
    }
    let delimiter = model(value)?;
    if format == "mathml" && delimiter.class != "Math::Symbols::Symbol" {
        let id = delimiter
            .class
            .strip_prefix("Math::Symbols::")
            .ok_or("unported promised table delimiter model")?;
        return crate::symbols::tables()["mathml"][id]["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "missing source MathML table delimiter descriptor".into());
    }
    let glyph = super::symbol(&delimiter, format)?;
    // Source OMML sets an attribute through its XML adapter, which decodes
    // numeric character references before serializing the attribute value.
    crate::xml_parse::source_entity_decode(&glyph)
        .map_err(|_| "unported source table delimiter entity".into())
}
fn cell_nodes(
    cell: &Model,
    format: &str,
    spacing: bool,
    templates: bool,
) -> Result<Vec<Xml>, String> {
    let mut content = Vec::new();
    for value in values(cell.field("parameter_one"), "table cell")? {
        if value.is_null() {
            if format == "omml" {
                return Err("source_render_error: undefined method 'insert_t_tag' for nil".into());
            }
        } else {
            content.extend(render_xml(&model(value)?, format, spacing, templates)?);
        }
    }
    Ok(content)
}
fn cell(cell: &Model, format: &str, spacing: bool, templates: bool) -> Result<Vec<Xml>, String> {
    let items = values(cell.field("parameter_one"), "table cell")?;
    if format == "mathml" {
        if items.first().is_some_and(vertical_bar) {
            return Ok(Vec::new());
        }
        Ok(vec![attrs(
            "mtd",
            attribute_values(cell.field("parameter_two"))?,
            cell_nodes(cell, format, spacing, templates)?,
        )])
    } else {
        Ok(vec![el(
            "m:e",
            cell_nodes(cell, format, spacing, templates)?,
        )])
    }
}
fn row(row: &Model, format: &str, spacing: bool, templates: bool) -> Result<Vec<Xml>, String> {
    let cells = values(row.field("parameter_one"), "table row")?;
    let mut content = Vec::new();
    for (index, value) in cells.iter().enumerate() {
        let mut value = model(value)?;
        if format == "mathml" && index == 0 {
            let items = values(value.field("parameter_one"), "first table cell")?;
            if items
                .first()
                .is_some_and(|item| item["class"] == "Math::Symbols::Hline")
            {
                value
                    .fields
                    .insert("parameter_one".into(), Value::Array(items[1..].to_vec()));
            }
        }
        content.extend(render(&value, format, spacing, templates)?);
    }
    if format == "mathml" {
        Ok(vec![el("mtr", content)])
    } else if cells.len() == 1 {
        Ok(content)
    } else {
        Ok(vec![el("m:mr", content)])
    }
}
fn omml_table(
    table: &Model,
    rows: &[Value],
    spacing: bool,
    templates: bool,
) -> Result<Vec<Xml>, String> {
    let mut single = true;
    for value in rows {
        single &= values(model(value)?.field("parameter_one"), "table row")?.len() == 1;
    }
    single &= ["frame", "columnlines", "rowlines"]
        .iter()
        .all(|key| table.field("options")[*key].is_null());
    let mut content = Vec::new();
    let name;
    if single {
        name = "m:eqArr";
        content.push(el("m:eqArrPr", vec![control_properties()]));
    } else {
        name = "m:m";
        let count = values(
            model(rows.first().ok_or("unported source empty matrix")?)?.field("parameter_one"),
            "table row",
        )?
        .len();
        let properties = el(
            "m:mcPr",
            vec![
                attrs("m:count", vec![("m:val".into(), count.to_string())], vec![]),
                attrs("m:mcJc", vec![("m:val".into(), "center".into())], vec![]),
            ],
        );
        content.push(el(
            "m:mPr",
            vec![
                el("m:mcs", vec![el("m:mc", vec![properties])]),
                control_properties(),
            ],
        ));
    }
    for value in rows {
        content.extend(render(&model(value)?, "omml", spacing, templates)?);
    }
    let table_node = el(name, content);
    if table.field("open_paren").is_null() && table.field("close_paren").is_null() {
        return Ok(vec![table_node]);
    }
    let mut delimiter_properties = Vec::new();
    for (field, tag) in [("open_paren", "m:begChr"), ("close_paren", "m:endChr")] {
        if !table.field(field).is_null() {
            delimiter_properties.push(attrs(
                tag,
                vec![("m:val".into(), parenthesis(table.field(field), "omml")?)],
                vec![],
            ));
        }
    }
    delimiter_properties.push(attrs(
        "m:sepChr",
        vec![("m:val".into(), String::new())],
        vec![],
    ));
    delimiter_properties.push(el("m:grow", vec![]));
    Ok(vec![el(
        "m:d",
        vec![
            el("m:dPr", delimiter_properties),
            el("m:e", vec![table_node]),
        ],
    )])
}
fn table(table: &Model, format: &str, spacing: bool, templates: bool) -> Result<Vec<Xml>, String> {
    let rows = values(table.field("value"), "table")?;
    if format == "omml" {
        return omml_table(table, rows, spacing, templates);
    }
    let attributes = table_attributes(table, rows)?;
    let mut content = Vec::new();
    for value in rows {
        content.extend(render(&model(value)?, format, spacing, templates)?);
    }
    let table_node = attrs("mtable", attributes, content);
    if table.class == "Math::Function::Table::Array" {
        return Ok(vec![table_node]);
    }
    if table.class == "Math::Function::Table::Matrix"
        && (table.field("open_paren").is_null()
            || table.field("close_paren").is_null()
            || (table.field("open_paren")["class"] == "Math::Symbols::Paren::Lround"
                && table.field("close_paren")["class"] == "Math::Symbols::Paren::Rround"))
    {
        return Ok(vec![table_node]);
    }

    if table.field("open_paren")["class"] == "Math::Symbols::Paren::Norm" {
        return Ok(vec![el(
            "mrow",
            vec![
                el("mo", vec![text("&#x2016;")]),
                table_node,
                el("mo", vec![text("&#x2016;")]),
            ],
        )]);
    }
    let left = parenthesis(table.field("open_paren"), "mathml")?;
    let right = parenthesis(table.field("close_paren"), "mathml")?;
    if table.class == "Math::Function::Table::Bmatrix" || !left.is_empty() || !right.is_empty() {
        Ok(vec![el(
            "mrow",
            vec![
                el("mo", vec![text(&left)]),
                table_node,
                el("mo", vec![text(&right)]),
            ],
        )])
    } else {
        Ok(vec![table_node])
    }
}

// Formula XML insertion dereferences every element in the Ruby source.
// This differs from Td MathML's safe-navigation insertion of a direct nil.
fn reject_null_formula(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if object.get("class").and_then(Value::as_str) == Some("Math::Formula")
                && object
                    .get("fields")
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(Value::is_null))
            {
                return Err("unported source Formula XML refusal for a nil element".into());
            }
            for child in object.values() {
                reject_null_formula(child)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                reject_null_formula(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn render(
    model: &Model,
    format: &str,
    spacing: bool,
    templates: bool,
) -> Result<Vec<Xml>, String> {
    reject_null_formula(&model.value())?;
    match model.class.as_str() {
        "Math::Function::Table"
        | "Math::Function::Table::Matrix"
        | "Math::Function::Table::Pmatrix"
        | "Math::Function::Table::Bmatrix"
        | "Math::Function::Table::Vmatrix"
        | "Math::Function::Table::Array" => table(model, format, spacing, templates),
        "Math::Function::Tr" => row(model, format, spacing, templates),
        "Math::Function::Td" => cell(model, format, spacing, templates),
        _ => Err(format!("unported source table XML class {}", model.class)),
    }
}
