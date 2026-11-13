//! Source-backed ordinary unary/binary writers; no input-specific dispatch.
use super::{attrs, control_properties, el, text, Model, Value, WriterContext, Xml};
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn child(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    super::multiscripts::child(v, f, ctx)
}
fn nil_safe(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if v.is_null() {
        Ok(String::new())
    } else {
        child(v, f, ctx)
    }
}
fn parens(v: &Value, ctx: &WriterContext) -> Result<String, String> {
    super::final_options::parens(v, ctx)
}
fn wrapped(v: &Value, ctx: &WriterContext) -> Result<String, String> {
    if truthy(v) {
        Ok(format!("({})", child(v, "asciimath", ctx)?))
    } else {
        Ok(String::new())
    }
}
fn latex_wrapped(v: &Value, ctx: &WriterContext) -> Result<String, String> {
    super::nary::latex_wrapped_with_value(v, child(v, "latex", ctx)?)
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(m.kind(), "Sqrt" | "Lim" | "Overset" | "Stackrel" | "Obrace")
}
fn unary_value(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if let Some(values) = v.as_array() {
        let mut out = Vec::new();
        for value in values {
            if !value.is_null() {
                out.push(child(value, f, ctx)?);
            }
        }
        Ok(out.join(if f == "latex" { " " } else { "" }))
    } else {
        nil_safe(v, f, ctx)
    }
}
fn unary_html(v: &Value, name: &str, ctx: &WriterContext) -> Result<String, String> {
    let value = if let Some(values) = v.as_array() {
        let mut out = String::new();
        for value in values {
            out.push_str(&child(value, "html", ctx)?);
        }
        format!("<i>{out}</i>")
    } else if truthy(v) {
        format!("<i>{}</i>", child(v, "html", ctx)?)
    } else {
        String::new()
    };
    Ok(format!("<i>{name}</i>{value}"))
}
fn contains_raw(table: &str, raw: &str) -> bool {
    let data = super::final_options::data();
    let extra: Value = serde_json::from_str(include_str!("portable_functions_data.json"))
        .expect("source Unicode declarations");
    let value = if table == "diacritic_overlays" {
        &extra[table]
    } else {
        &data[table]
    };
    match value {
        Value::Array(v) => v.iter().any(|x| x == raw),
        Value::Object(v) => v.values().any(|x| x == raw),
        _ => false,
    }
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    match (m.kind(), f) {
        ("Sqrt", "asciimath") => Ok(format!(
            "sqrt{}",
            if truthy(a) {
                format!("({})", unary_value(a, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Sqrt", "latex") => Ok(format!("\\sqrt{{{}}}", unary_value(a, f, ctx)?)),
        ("Sqrt", "html") => unary_html(a, "sqrt", ctx),
        ("Sqrt", "unicodemath") => Ok(format!("√{}", parens(a, ctx)?)),
        ("Lim", "asciimath") => Ok(format!(
            "lim{}{}",
            if truthy(a) {
                format!("_{}", wrapped(a, ctx)?)
            } else {
                String::new()
            },
            if truthy(b) {
                format!("^{}", wrapped(b, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Lim", "latex") => Ok(format!(
            "\\lim{}{}",
            if truthy(a) {
                format!("_{{{}}}", child(a, f, ctx)?)
            } else {
                String::new()
            },
            if truthy(b) {
                format!("^{{{}}}", child(b, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Lim", "html") => Ok(format!(
            "<i>lim</i>{}{}",
            if truthy(a) {
                format!("<i>{}</i>", child(a, f, ctx)?)
            } else {
                String::new()
            },
            if truthy(b) {
                format!("<i>{}</i>", child(b, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Lim", "unicodemath") => Ok(format!(
            "lim{}{}",
            if truthy(a) {
                format!("_{}", parens(a, ctx)?)
            } else {
                String::new()
            },
            if truthy(b) {
                format!("^{}", parens(b, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Overset", "asciimath") => Ok(format!("overset{}{}", wrapped(a, ctx)?, wrapped(b, ctx)?)),
        ("Overset" | "Stackrel", "latex") => Ok(format!(
            "\\{}{}{}",
            m.kind().to_lowercase(),
            if truthy(a) {
                latex_wrapped(a, ctx)?
            } else {
                String::new()
            },
            if truthy(b) {
                latex_wrapped(b, ctx)?
            } else {
                String::new()
            }
        )),
        ("Overset", "html") => Ok(format!(
            "{}{}",
            if truthy(a) {
                format!("<i>{}</i>", child(a, f, ctx)?)
            } else {
                String::new()
            },
            if truthy(b) {
                format!("<i>{}</i>", child(b, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Overset", "unicodemath") => {
            let raw_a = super::final_options::raw_symbol(a)?;
            let raw_b = super::final_options::raw_symbol(b)?;
            if raw_a.as_ref().is_some_and(|v| {
                contains_raw("diacritic_overlays", v) || contains_raw("accent_symbols", v)
            }) {
                return Ok(format!("{}{}", parens(b, ctx)?, raw_a.unwrap()));
            }
            if raw_b.as_ref().is_some_and(|v| {
                contains_raw("diacritic_overlays", v) || contains_raw("accent_symbols", v)
            }) {
                return Ok(format!(
                    "{}{}",
                    raw_a
                        .ok_or("source_render_error: nil Unicode accent interpolation receiver")?,
                    child(b, f, ctx)?
                ));
            }
            if raw_a
                .as_ref()
                .is_some_and(|v| contains_raw("horizontal_brackets", v))
            {
                return Ok(format!("{}{}", child(a, f, ctx)?, parens(b, ctx)?));
            }
            if matches!(
                b["class"].as_str(),
                Some("Math::Function::Obrace" | "Math::Function::Ubrace")
            ) {
                return Ok(format!("{}^{}", child(b, f, ctx)?, parens(a, ctx)?));
            }
            Ok(if truthy(a) || truthy(b) {
                format!("{}┴{}", parens(b, ctx)?, parens(a, ctx)?)
            } else {
                String::new()
            })
        }
        ("Stackrel", "asciimath") => {
            let wrap = |v: &Value| -> Result<String, String> {
                let s = nil_safe(v, f, ctx)?;
                Ok(if s.starts_with('(') {
                    s
                } else {
                    format!("({s})")
                })
            };
            Ok(format!("stackrel{}{}", wrap(a)?, wrap(b)?))
        }
        ("Stackrel", "html") => Ok(format!("{}{}", nil_safe(a, f, ctx)?, nil_safe(b, f, ctx)?)),
        ("Stackrel", "unicodemath") => Ok(format!(
            "({})┴({})",
            nil_safe(b, f, ctx)?,
            nil_safe(a, f, ctx)?
        )),
        ("Obrace", "asciimath") => Ok(format!(
            "obrace{}",
            if truthy(a) {
                format!("({})", child(a, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Obrace", "latex") => Ok(format!(
            "\\overbrace{}",
            if truthy(a) {
                format!("{{{}}}", child(a, f, ctx)?)
            } else {
                String::new()
            }
        )),
        ("Obrace", "html") => unary_html(a, "&#x23de;", ctx),
        ("Obrace", "unicodemath") => Ok(format!("⏞({})", nil_safe(a, f, ctx)?)),
        _ => Err(format!("unported portable function text {} {f}", m.class)),
    }
}
fn xml_child(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    super::multiscripts::xml_child(v, f, ctx)
}
fn omml_parameter(v: &Value, tag: &str, ctx: &WriterContext) -> Result<Xml, String> {
    if !truthy(v) {
        return Ok(el(
            tag,
            vec![el("m:r", vec![el("m:t", vec![text("&#8203;")])])],
        ));
    }
    let mut output = Vec::new();
    if let Some(values) = v.as_array() {
        for value in values {
            if value.is_null() || value == false {
                return Err("source_render_error: array entry has no insert_t_tag method".into());
            }
            output.extend(xml_child(value, "omml", ctx)?);
        }
    } else {
        output = xml_child(v, "omml", ctx)?;
    }
    Ok(el(tag, output))
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    if m.kind() == "Sqrt" {
        if f == "mathml" {
            return Ok(vec![el(
                "msqrt",
                if a.is_null() {
                    vec![]
                } else {
                    xml_child(a, f, ctx)?
                },
            )]);
        }
        return Ok(vec![el(
            "m:rad",
            vec![
                el(
                    "m:radPr",
                    vec![
                        attrs("m:degHide", vec![("m:val".into(), "on".into())], vec![]),
                        control_properties(),
                    ],
                ),
                el("m:deg", vec![]),
                omml_parameter(a, "m:e", ctx)?,
            ],
        )]);
    }
    if m.kind() == "Stackrel" {
        if f == "mathml" {
            let mut nodes = Vec::new();
            for value in [b, a] {
                nodes.push(el(
                    "mrow",
                    if value.is_null() {
                        vec![text("")]
                    } else {
                        xml_child(value, f, ctx)?
                    },
                ));
            }
            return Ok(vec![el("mover", nodes)]);
        }
        return Ok(vec![el(
            "m:limUpp",
            vec![
                el("m:limUppPr", vec![control_properties()]),
                omml_parameter(b, "m:e", ctx)?,
                omml_parameter(a, "m:lim", ctx)?,
            ],
        )]);
    }
    if m.kind() == "Obrace" {
        if f == "mathml" {
            let mo = el("mo", vec![text("&#x23de;")]);
            if !truthy(a) {
                return Ok(vec![mo]);
            }
            let attributes = super::underline::attribute_values(m, ctx)?;
            let mut body = xml_child(a, f, ctx)?;
            body.push(mo);
            return Ok(vec![attrs("mover", attributes, body)]);
        }
        if !truthy(a) {
            return Ok(vec![el("m:r", vec![el("m:t", vec![text("⏞")])])]);
        }
        if truthy(&m.field("attributes")["accent"]) {
            return Ok(vec![el(
                "m:acc",
                vec![
                    el(
                        "m:accPr",
                        vec![attrs("m:chr", vec![("m:val".into(), "⏞".into())], vec![])],
                    ),
                    el("m:e", xml_child(a, f, ctx)?),
                ],
            )]);
        }
        let over = Model::from_value(
            &serde_json::json!({"class":"Math::Function::Overset","fields":{"parameter_one":{"class":"Math::Symbols::Symbol","fields":{"value":"⏞"}},"parameter_two":a}}),
        )?;
        return super::render_xml(&over, f, &ctx.with_display_style(true));
    }
    Err(format!("unported portable function XML {}", m.class))
}
