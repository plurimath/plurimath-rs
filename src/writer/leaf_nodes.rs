//! Sup/Root/Ms source procedures; object-address interpolation is explicitly unsupported.
use super::{attrs, el, text, Model, Value, WriterContext, Xml};
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn child(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    super::multiscripts::child(v, f, ctx)
}
fn safe(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if v.is_null() {
        Ok(String::new())
    } else {
        child(v, f, ctx)
    }
}
fn compact_text(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if let Some(values) = v.as_array() {
        values
            .iter()
            .filter(|v| !v.is_null())
            .map(|v| child(v, f, ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(|s| s.join(if f == "latex" { " " } else { "" }))
    } else {
        safe(v, f, ctx)
    }
}
fn html_unary(m: &Model, name: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let v = m.field("parameter_one");
    let body = if let Some(values) = v.as_array() {
        Some(
            values
                .iter()
                .map(|v| child(v, "html", ctx))
                .collect::<Result<Vec<_>, _>>()?
                .join(""),
        )
    } else if truthy(v) {
        Some(child(v, "html", ctx)?)
    } else {
        None
    };
    Ok(format!(
        "<i>{name}</i>{}",
        body.map(|s| format!("<i>{s}</i>")).unwrap_or_default()
    ))
}
fn contains_source_node(value: &Value) -> bool {
    match value {
        Value::Object(fields) => {
            fields.contains_key("class") && fields.contains_key("fields")
                || fields.values().any(contains_source_node)
        }
        Value::Array(values) => values.iter().any(contains_source_node),
        _ => false,
    }
}
fn ruby_string(v: &Value) -> Result<String, String> {
    if contains_source_node(v) {
        return Err("unported promised Ruby source object interpolation/inspection".into());
    }
    use crate::source_values::{decode, to_s, Domain, Profile};
    to_s(&decode(v, Domain::Literal, Profile::Canonical)?)
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Sup" | "Math::Function::Root" | "Math::Function::Ms"
    )
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    match m.kind() {
        "Sup" => match f {
            "asciimath" => Ok(format!(
                "sup{}",
                if truthy(a) {
                    compact_text(a, f, ctx)?
                } else {
                    String::new()
                }
            )),
            "latex" => Ok(format!("\\sup{{{}}}", compact_text(a, f, ctx)?)),
            "html" => html_unary(m, "&#x2283;", ctx),
            "unicodemath" => Ok(format!("sup⁡{}", safe(a, f, ctx)?)),
            _ => Err("unported Sup format".into()),
        },
        "Root" => match f {
            "asciimath" => {
                let mut out = "root".to_string();
                for v in [a, b] {
                    if truthy(v) {
                        out.push_str(&format!("({})", child(v, f, ctx)?));
                    }
                }
                Ok(out)
            }
            "latex" => Ok(format!("\\sqrt[{}]{{{}}}", safe(a, f, ctx)?, safe(b, f, ctx)?)),
            "html" => {
                let mut out = String::new();
                for v in [a, b] {
                    if truthy(v) {
                        out.push_str(&format!("<i>{}</i>", child(v, f, ctx)?));
                    }
                }
                Ok(out)
            }
            "unicodemath" => Ok(format!(
                "√({}&{})",
                if truthy(a) {
                    child(a, f, ctx)?
                } else {
                    String::new()
                },
                if truthy(b) {
                    child(b, f, ctx)?
                } else {
                    String::new()
                }
            )),
            _ => Err("unported Root format".into()),
        },
        "Ms" => match f {
            "asciimath" => Ok(format!("\"“{}”\"", ruby_string(a)?)),
            "latex" => Ok(format!("\\text{{“{}”}}", ruby_string(a)?)),
            "html" => html_unary(m, "ms", ctx),
            "unicodemath" => {
                let node = serde_json::json!({"class":"Math::Function::Text","fields":{"parameter_one":a}});
                super::render_text(&Model::from_value(&node)?, f, ctx)
            }
            _ => Err("unported Ms format".into()),
        },
        _ => Err("unported leaf class".into()),
    }
}
fn direct(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if v.is_null() {
        Ok(vec![])
    } else {
        super::multiscripts::xml_child(v, f, ctx)
    }
}
fn compact_xml(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if let Some(values) = v.as_array() {
        let mut out = vec![];
        for v in values.iter().filter(|v| !v.is_null()) {
            out.extend(direct(v, f, ctx)?);
        }
        Ok(out)
    } else {
        direct(v, f, ctx)
    }
}
fn omml_parameter(v: &Value, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if !truthy(v) {
        return Ok(vec![super::run("&#8203;")]);
    }
    if let Some(values) = v.as_array() {
        let mut out = vec![];
        for value in values {
            out.extend(super::multiscripts::xml_child(value, "omml", ctx)?);
        }
        Ok(out)
    } else {
        direct(v, "omml", ctx)
    }
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    match m.kind() {
        "Sup" => {
            let mut body = compact_xml(a, f, ctx)?;
            if f == "mathml" {
                if !truthy(m.field("hide_function_name")) {
                    body.insert(0, el("mo", vec![text("sup")]));
                }
                Ok(vec![el("mrow", body)])
            } else {
                if !truthy(m.field("hide_function_name")) {
                    body.insert(0, el("m:r", vec![el("m:t", vec![text("sup")])]));
                }
                Ok(body)
            }
        }
        "Root" => {
            if f == "mathml" {
                let mut body = direct(b, f, ctx)?;
                body.extend(direct(a, f, ctx)?);
                Ok(vec![el("mroot", body)])
            } else {
                Ok(vec![el(
                    "m:rad",
                    vec![
                        el(
                            "m:radPr",
                            vec![attrs(
                                "m:degHide",
                                vec![("m:val".into(), "off".into())],
                                vec![],
                            )],
                        ),
                        el("m:deg", omml_parameter(a, ctx)?),
                        el("m:e", omml_parameter(b, ctx)?),
                    ],
                )])
            }
        }
        "Ms" => {
            if f == "mathml" {
                Ok(vec![el(
                    "ms",
                    vec![text(a.as_str().ok_or(
                        "source_render_error: ms XML append requires a String",
                    )?)],
                )])
            } else {
                Ok(vec![el(
                    "m:t",
                    vec![text(&format!("“{}”", ruby_string(a)?))],
                )])
            }
        }
        _ => Err("unported leaf XML class".into()),
    }
}
