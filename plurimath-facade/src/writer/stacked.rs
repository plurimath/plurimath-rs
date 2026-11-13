//! Source Substack/Msline/Mglyph and row/cell text procedures (Ruby 0a1df68a).
use super::{attrs, child, el, render_xml, text as xml_text, Model, Value, WriterContext, Xml};
fn source_error(reason: &str) -> String {
    format!("source_render_error: {reason}")
}
fn row_values(v: &Value) -> Result<&[Value], String> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| source_error("stacked row/cell collection has no source array methods"))
}
fn node_text(
    v: &Value,
    f: &str,
    nullable: bool,
    ctx: &super::WriterContext,
) -> Result<String, String> {
    if v.is_null() && nullable {
        return Ok(String::new());
    }
    if !v.is_object() || v.get("class").is_none() {
        return Err(source_error("stacked child has no source writer"));
    }
    child(v, f, ctx)
}
fn vertical(v: &Value) -> bool {
    v["class"] == "Math::Symbols::Paren::Vert"
        || (v["class"] == "Math::Symbols::Symbol" && v["fields"]["value"] == "|")
}
fn row_cell_text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let values = row_values(m.field("parameter_one"))?;
    let row = m.kind() == "Tr";
    let mut parts = Vec::new();
    for v in values {
        if row && f == "latex" && (vertical(v) || vertical(&v["fields"]["parameter_one"][0])) {
            continue;
        }
        if !row && f == "latex" && vertical(&values.first().cloned().unwrap_or(Value::Null)) {
            return Ok(String::new());
        }
        parts.push(node_text(v, f, !row && f != "html", ctx)?);
    }
    let sep = if row {
        match f {
            "asciimath" => ", ",
            "latex" => " & ",
            "unicodemath" => "&",
            _ => "",
        }
    } else {
        match f {
            "asciimath" | "latex" => " ",
            _ => "",
        }
    };
    let body = parts.join(sep);
    Ok(match (m.kind(), f) {
        ("Tr", "asciimath") => format!("[{body}]"),
        ("Tr", "html") => format!("<tr>{body}</tr>"),
        ("Td", "html") => format!("<td>{body}</td>"),
        _ => body,
    })
}
fn unary_html(m: &Model, ctx: &super::WriterContext) -> Result<String, String> {
    let p = m.field("parameter_one");
    if p.is_null() || p == false {
        return Ok(format!("<i>{}</i>", m.kind().to_lowercase()));
    }
    let body = if let Some(values) = p.as_array() {
        values
            .iter()
            .map(|v| node_text(v, "html", false, ctx))
            .collect::<Result<Vec<_>, _>>()?
            .join("")
    } else {
        node_text(p, "html", false, ctx)?
    };
    Ok(format!("<i>{}</i><i>{body}</i>", m.kind().to_lowercase()))
}
fn string(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err("unported source glyph array/hash scalar coercion".into()),
    }
}
pub(super) fn handles_text(m: &Model) -> bool {
    matches!(m.kind(), "Substack" | "Msline" | "Mglyph" | "Tr" | "Td")
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if ["Tr", "Td"].contains(&m.kind()) {
        return row_cell_text(m, f, ctx);
    }
    if f == "html" {
        return unary_html(m, ctx);
    }
    if m.kind() == "Msline" {
        return Ok(String::new());
    }
    if m.kind() == "Mglyph" {
        let p = m.field("parameter_one");
        if f == "unicodemath" && p.is_null() {
            return Ok(String::new());
        }
        return p
            .as_object()
            .ok_or_else(|| source_error("glyph attributes have no source lookup"))
            .and_then(|p| string(p.get("alt").unwrap_or(&Value::Null)));
    }
    let p = m.field("parameter_one");
    let mut parts = Vec::new();
    if !p.is_null() {
        for v in row_values(p)? {
            if !v.is_null() {
                parts.push(node_text(v, f, false, ctx)?);
            }
        }
    }
    Ok(match f {
        "asciimath" => format!("{{:{}:}}", parts.join(",")),
        "latex" => format!("\\substack{{{}}}", parts.join(" \\\\ ")),
        "unicodemath" => format!("■({})", parts.join("@")),
        _ => return Err("unknown stacked text format".into()),
    })
}
// Ruby String#to_i consumes a signed decimal prefix. Decimal division preserves
// arbitrary-sized indexes instead of truncating them through a machine integer.
fn index_decimal(v: &Value) -> Result<(bool, String), String> {
    let s = match v {
        Value::Null => return Ok((false, "0".into())),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let s = n.to_string();
            if s.contains(['.', 'e', 'E']) {
                let f = n.as_f64().ok_or("unported non-finite glyph index")?;
                format!("{:.0}", f.trunc())
            } else {
                s
            }
        }
        _ => return Err(source_error("glyph index has no source to_i")),
    };
    let s = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let negative = s.starts_with('-');
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let mut digits = String::new();
    let mut last_digit = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            last_digit = true;
        } else if c == '_' && last_digit {
            last_digit = false;
        } else {
            break;
        }
    }
    let normalized = digits.trim_start_matches('0');
    Ok((
        negative,
        if normalized.is_empty() {
            "0".into()
        } else {
            normalized.into()
        },
    ))
}
fn decimal_hex(mut digits: String) -> String {
    let mut out = Vec::new();
    while digits != "0" {
        let mut next = String::new();
        let mut remainder = 0u32;
        for b in digits.bytes() {
            let n = remainder * 10 + u32::from(b - b'0');
            let d = n / 16;
            remainder = n % 16;
            if d != 0 || !next.is_empty() {
                next.push(char::from(b'0' + d as u8));
            }
        }
        out.push(b"0123456789ABCDEF"[remainder as usize] as char);
        digits = if next.is_empty() { "0".into() } else { next };
    }
    out.into_iter().rev().collect()
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    match m.kind() {
        "Msline" => Ok(if f == "mathml" {
            vec![el("msline", vec![])]
        } else {
            vec![]
        }),
        "Mglyph" => {
            let p = m
                .field("parameter_one")
                .as_object()
                .ok_or_else(|| source_error("glyph attributes have no source lookup"))?;
            if f == "mathml" {
                let a = p
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), string(v)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                return Ok(vec![attrs("mglyph", a, vec![])]);
            }
            let (negative, index) = index_decimal(p.get("index").unwrap_or(&Value::Null))?;
            let small = index.parse::<u32>().ok();
            let ignored = negative
                || small.is_some_and(|i| {
                    i == 0 || (i < 32 && ![9, 10, 13].contains(&i)) || [65534, 65535].contains(&i)
                });
            let value = if ignored {
                string(p.get("alt").unwrap_or(&Value::Null))?
            } else {
                format!("&#x{};", decimal_hex(index))
            };
            Ok(vec![el("m:r", vec![el("m:t", vec![xml_text(&value)])])])
        }
        "Substack" => {
            let p = m.field("parameter_one");
            if f == "omml" {
                let rows = row_values(p)?;
                if rows
                    .iter()
                    .any(|v| v.is_null() || v["class"] != "Math::Function::Tr")
                {
                    return Err(source_error("source Table requires row parameter methods"));
                }
                let table = Model::from_value(
                    &serde_json::json!({"class":"Math::Function::Table","fields":{"value":rows,"options":{},"open_paren":null,"close_paren":null}}),
                )?;
                return super::table_xml::render(&table, f, ctx);
            }
            let mut content = Vec::new();
            if let Some(values) = p.as_array() {
                for v in values {
                    if !v.is_null() {
                        let child = Model::from_value(v)
                            .map_err(|_| source_error("stacked child has no source XML writer"))?;
                        content.extend(render_xml(&child, f, ctx)?);
                    }
                }
            } else if !p.is_null() {
                let child = Model::from_value(p)
                    .map_err(|_| source_error("stacked child has no source XML writer"))?;
                content.extend(render_xml(&child, f, ctx)?);
            }
            Ok(vec![el("mtable", content)])
        }
        _ => Err("unknown stacked XML node".into()),
    }
}
