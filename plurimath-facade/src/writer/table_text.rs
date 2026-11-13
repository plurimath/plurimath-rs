//! Generic Table/Tr/Td text procedures from Ruby0a1d; subclasses retain their own writers.
use super::{child, formatter_context, Model, Value, WriterContext};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("table_text_data.json")).unwrap())
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Table" | "Math::Function::Tr" | "Math::Function::Td"
    )
}
fn error(ctx: &WriterContext, method: &str) -> String {
    formatter_context::source_error(
        ctx,
        "NoMethodError",
        format!("undefined source writer method {method} on a non-node table value"),
    )
}
fn array<'a>(
    value: &'a Value,
    ctx: &WriterContext,
    allow_nil: bool,
) -> Result<Option<&'a Vec<Value>>, String> {
    if value.is_null() && allow_nil {
        return Ok(None);
    }
    value.as_array().map(Some).ok_or_else(|| error(ctx, "map"))
}
fn node(value: &Value, f: &str, ctx: &WriterContext, safe_nil: bool) -> Result<String, String> {
    if value.is_null() && safe_nil {
        return Ok(String::new());
    }
    if !value.is_object() {
        return Err(error(ctx, &format!("to_{f}")));
    }
    child(value, f, ctx)
}
fn vert(value: &Value) -> bool {
    value["class"] == "Math::Symbols::Paren::Vert"
        || (value["class"] == "Math::Symbols::Symbol" && value["fields"]["value"] == "|")
}
fn sequence(
    values: Option<&Vec<Value>>,
    f: &str,
    ctx: &WriterContext,
    safe_nil: bool,
    separator: &str,
) -> Result<String, String> {
    let mut parts = vec![];
    if let Some(values) = values {
        for value in values {
            parts.push(node(value, f, ctx, safe_nil)?);
        }
    }
    Ok(parts.join(separator))
}
fn cell(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let values = array(m.field("parameter_one"), ctx, f == "unicodemath")?;
    if f == "latex" && values.and_then(|v| v.first()).is_some_and(vert) {
        return Ok(String::new());
    }
    let mut parts = vec![];
    if let Some(values) = values {
        for value in values {
            let scoped;
            let call = if f == "asciimath" && value["class"] == "Math::Formula" {
                scoped = ctx.with_table();
                &scoped
            } else {
                ctx
            };
            parts.push(node(value, f, call, f != "html")?);
        }
    }
    let body = parts.join(if f == "asciimath" || f == "latex" {
        " "
    } else {
        ""
    });
    Ok(if f == "html" {
        format!("<td>{body}</td>")
    } else {
        body
    })
}
fn row(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let values = array(m.field("parameter_one"), ctx, f == "unicodemath")?;
    let mut parts = vec![];
    if let Some(values) = values {
        for value in values {
            if f == "latex" {
                if vert(value) {
                    continue;
                }
                let first = value["fields"]["parameter_one"]
                    .as_array()
                    .and_then(|v| v.first());
                if first.is_some_and(vert) {
                    continue;
                }
                if !value["fields"]["parameter_one"].is_array() {
                    return Err(error(ctx, "parameter_one.first"));
                }
            }
            parts.push(node(value, f, ctx, false)?);
        }
    }
    let body = parts.join(match f {
        "asciimath" => ", ",
        "latex" => " & ",
        "unicodemath" => "&",
        _ => "",
    });
    Ok(match f {
        "asciimath" => format!("[{body}]"),
        "html" => format!("<tr>{body}</tr>"),
        _ => body,
    })
}
fn invisible_close(close: &Value) -> bool {
    close["class"] == "Math::Symbols::Paren::CloseParen"
        || (close["class"] == "Math::Symbols::Symbol" && close["fields"]["value"] == "&#x2524;")
}
fn table(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let rows = array(m.field("value"), ctx, f != "html")?;
    let open = m.field("open_paren");
    let close = m.field("close_paren");
    match f {
        "html" => Ok(format!(
            "<table>{}</table>",
            sequence(rows, f, ctx, false, "")?
        )),
        "asciimath" => {
            let left = if open.is_null() {
                "[".into()
            } else {
                node(open, f, ctx, false)?
            };
            let right = if close.is_null() {
                data()["asciimath_table_parenthesis"][&left]
                    .as_str()
                    .unwrap_or("")
                    .into()
            } else {
                node(close, f, ctx, false)?
            };
            Ok(format!(
                "{left}{}{right}",
                sequence(rows, f, ctx, true, ", ")?
            ))
        }
        "latex" => {
            let body = sequence(rows, f, ctx, true, " \\\\ ")?;
            if open["class"] == "Math::Symbols::Paren::Norm" {
                return Ok(format!("\\begin{{Vmatrix}}{body}\\end{{Vmatrix}}"));
            }
            let left = if open.is_null() {
                ".".into()
            } else {
                node(open, f, ctx, false)?
            };
            let right = if close.is_null() {
                ".".into()
            } else {
                node(close, f, ctx, false)?
            };
            Ok(format!(
                "\\left {}\\begin{{matrix}}{body}\\end{{matrix}}\\right {}",
                left.strip_prefix("\\left").unwrap_or(&left),
                right.strip_prefix("\\right").unwrap_or(&right)
            ))
        }
        "unicodemath" => {
            let body = sequence(rows, f, ctx, true, "@")?;
            let left = if open.is_null() {
                String::new()
            } else {
                node(open, f, ctx, false)?
            };
            if open.is_null() && !close.is_null() && !invisible_close(close) {
                return Err(error(ctx, "class_name"));
            }
            let source_field = if open["class"] == "Math::Symbols::Symbol" {
                open["fields"]["value"].as_str()
            } else {
                open["class"]
                    .as_str()
                    .and_then(|class| data()["unicode_field_hex"][class].as_str())
            };
            let named = !invisible_close(close)
                && (!open.is_null() || !close.is_null())
                && ((!open.is_null() && !close.is_null())
                    || (close.is_null()
                        && !source_field.is_some_and(|field| {
                            data()["utility_parenthesis_keys"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|v| v == field)
                        })));
            if named {
                let name = if vert(open) && vert(close) {
                    Some("vmatrix")
                } else {
                    matrix_name(&left)
                };
                let mark = name
                    .and_then(|name| {
                        crate::symbols::tables()["unicode_matrix_marks"][name].as_str()
                    })
                    .unwrap_or("");
                return Ok(format!("{}({body})", super::text_content::decode(mark)?));
            }
            let right = if invisible_close(close) {
                "┤".into()
            } else if close.is_null() {
                String::new()
            } else {
                node(close, f, ctx, false)?
            };
            Ok(format!("{left}■({body}){right}"))
        }
        _ => Err(format!("unknown table text format {f}")),
    }
}
pub(super) fn render(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    match m.class.as_str() {
        "Math::Function::Table" => table(m, f, ctx),
        "Math::Function::Tr" => row(m, f, ctx),
        "Math::Function::Td" => cell(m, f, ctx),
        _ => Err("not a source table text node".into()),
    }
}

fn matrix_name(value: &str) -> Option<&'static str> {
    crate::symbols::tables()["unicode_matrix_parens"]
        .as_object()?
        .iter()
        .find_map(|(key, paren)| {
            if paren == value {
                Some(key.as_str())
            } else {
                None
            }
        })
}
