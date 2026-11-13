//! Default source table/row/cell text writers. Options requiring a new profile refuse.
use super::{child, text_content, Model, Value};
fn rows(m: &Model) -> Result<Vec<Vec<Model>>, String> {
    if m.field("options").as_object().is_none_or(|o| {
        !o.is_empty()
            && (!["Align", "Multline", "Split"].contains(&m.kind())
                || o.keys().any(|key| key != "asterisk"))
    }) {
        return Err("unported source table options".into());
    }
    let values = m
        .field("value")
        .as_array()
        .ok_or("unported non-list table rows")?;
    let mut out = vec![];
    for row in values {
        let row = Model::from_value(row)?;
        if row.class != "Math::Function::Tr" {
            return Err("unported non-Tr table row".into());
        }
        let cells = row
            .field("parameter_one")
            .as_array()
            .ok_or("unported non-list table cells")?;
        let mut c = vec![];
        for value in cells {
            let cell = Model::from_value(value)?;
            if cell.class != "Math::Function::Td" {
                return Err("unported non-Td table cell".into());
            }
            if !cell.field("parameter_one").is_array() {
                return Err("unported non-list Td contents".into());
            }
            let first = &cell.field("parameter_one")[0];
            if first["class"] == "Math::Symbols::Paren::Vert" {
                return Err("unported source table vertical separator cell".into());
            }
            let options = cell.field("parameter_two");
            if !options.is_null()
                && options
                    .as_object()
                    .is_none_or(|o| o.keys().any(|k| k != "columnalign"))
            {
                return Err("unported source Td attributes".into());
            }
            c.push(cell)
        }
        if c.is_empty() {
            return Err("unported empty source table row".into());
        }
        out.push(c)
    }
    if out.is_empty() {
        return Err("unported empty source table".into());
    }
    Ok(out)
}
fn reject_null_formula_values(value: &Value) -> Result<(), String> {
    if value["class"] == "Math::Formula"
        && value["fields"]["value"]
            .as_array()
            .is_some_and(|values| values.iter().any(Value::is_null))
    {
        return Err("unported source table Formula with nil operand (source writer raises)".into());
    }
    match value {
        Value::Array(values) => {
            for value in values {
                reject_null_formula_values(value)?;
            }
        }
        Value::Object(fields) => {
            for value in fields.values() {
                reject_null_formula_values(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn cell(c: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    reject_null_formula_values(c.field("parameter_one"))?;
    c.field("parameter_one")
        .as_array()
        .unwrap()
        .iter()
        .map(|v| child(v, f, ctx))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| {
            v.join(if f == "asciimath" || f == "latex" {
                " "
            } else {
                ""
            })
        })
}
pub(super) fn render(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if ![
        "Align", "Multline", "Split", "Matrix", "Pmatrix", "Bmatrix", "Vmatrix", "Array",
    ]
    .contains(&m.kind())
    {
        return Err("unported promised table subclass".into());
    }
    let rs = rows(m)?;
    let cells = rs
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| cell(c, f, ctx))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let open = m.field("open_paren");
    let close = m.field("close_paren");
    match f {
        "html" => Ok(format!(
            "<table>{}</table>",
            cells
                .iter()
                .map(|r| format!(
                    "<tr>{}</tr>",
                    r.iter()
                        .map(|c| format!("<td>{c}</td>"))
                        .collect::<String>()
                ))
                .collect::<String>()
        )),
        "asciimath" => {
            let body = cells
                .iter()
                .map(|r| format!("[{}]", r.join(", ")))
                .collect::<Vec<_>>()
                .join(", ");
            if open.is_null() && close.is_null() && m.kind() == "Multline" {
                return Ok(format!("[{body}]"));
            }
            if open.is_null() && ["Align", "Split", "Matrix", "Array"].contains(&m.kind()) {
                return Ok(format!("{{:{body}:}}"));
            }
            if open.is_null() || close.is_null() {
                return Err("unported missing table delimiters".into());
            }
            Ok(format!(
                "{}{body}{}",
                child(open, f, ctx)?,
                child(close, f, ctx)?
            ))
        }
        "latex" => {
            let mut env = if m.kind() == "Array" {
                "array".to_owned()
            } else if open.is_null() {
                m.kind().to_lowercase()
            } else {
                let id = open["class"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("Math::Symbols::"))
                    .ok_or("unported table delimiter class")?;
                crate::symbols::tables()["latex_matrix_environments"][id]
                    .as_str()
                    .ok_or("unported source matrix environment")?
                    .to_owned()
            };
            let source_options = crate::source_values::field(
                m,
                "options",
                crate::source_values::Domain::NamedOptions,
            )?;
            let asterisk = source_options.get_symbol("asterisk");
            if asterisk.is_some() {
                env.push('*');
            }
            let args = if m.kind() == "Array" {
                let mut a = String::new();
                for c in &rs[0] {
                    a.push(
                        match c.field("parameter_two")["columnalign"]
                            .as_str()
                            .unwrap_or("center")
                        {
                            "center" => 'c',
                            "left" => 'l',
                            "right" => 'r',
                            _ => return Err("unported source Array alignment".into()),
                        },
                    )
                }
                format!("{{{a}}}")
            } else if !matches!(
                asterisk,
                None | Some(crate::source_values::RubyValue::Nil)
                    | Some(crate::source_values::RubyValue::Bool(false))
            ) {
                let alignment = rs[0][0].field("parameter_two")["columnalign"]
                    .as_str()
                    .unwrap_or("");
                let letter = match alignment {
                    "left" => "l",
                    "center" => "c",
                    "right" => "r",
                    _ => "",
                };
                format!("[{letter}]")
            } else {
                String::new()
            };
            let body = cells
                .iter()
                .map(|r| r.join(" & "))
                .collect::<Vec<_>>()
                .join(" \\\\ ");
            Ok(format!("\\begin{{{env}}}{args}{body}\\end{{{env}}}"))
        }
        "unicodemath" => {
            let matrix = if open.is_null()
                && ["Align", "Multline", "Split", "Matrix", "Array"].contains(&m.kind())
            {
                String::from("matrix")
            } else {
                let paren = child(open, "unicodemath", ctx)?;
                crate::symbols::tables()["unicode_matrix_parens"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .find_map(|(k, v)| v.as_str().filter(|v| *v == paren).map(|_| k.clone()))
                    .ok_or("unported Unicode matrix delimiter")?
            };
            let mark = crate::symbols::tables()["unicode_matrix_marks"][&matrix]
                .as_str()
                .ok_or("unported Unicode matrix marker")?;
            let mark = text_content::decode(mark)?;
            Ok(format!(
                "{mark}({})",
                cells
                    .iter()
                    .map(|r| r.join("&"))
                    .collect::<Vec<_>>()
                    .join("@")
            ))
        }
        _ => Err("unported table writer format".into()),
    }
}
