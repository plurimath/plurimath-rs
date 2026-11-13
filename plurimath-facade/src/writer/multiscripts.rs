//! Source Multiscript procedures, including inherited HTML and prescript quirks.
use super::{attrs, el, render_text, render_xml, run, Model, Value, WriterContext, Xml};

fn truthy(value: &Value) -> bool {
    !value.is_null() && value != false
}
fn node(value: &Value) -> Result<Model, String> {
    Model::from_value(value).map_err(|error| source_error(&error))
}
pub(super) fn child(
    value: &Value,
    format: &str,
    ctx: &super::WriterContext,
) -> Result<String, String> {
    let m = node(value)?;
    // None is the explicit source Multiscript placeholder. Its four
    // textual methods return nil, which the source joins as empty text.
    if m.class == "Math::Function::None" {
        if format != "html" {
            return Ok(String::new());
        }
        let field = m.field("parameter_one");
        let mut output = "<i>none</i>".to_string();
        if let Some(values) = field.as_array() {
            let body = values
                .iter()
                .map(|v| child(v, format, ctx))
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            output.push_str(&format!("<i>{body}</i>"));
        } else if truthy(field) {
            output.push_str(&format!("<i>{}</i>", child(field, format, ctx)?));
        }
        return Ok(output);
    }
    if m.class == "Math::Function::Linebreak" {
        return linebreak_text(&m, format, ctx);
    }
    render_text(&m, format, ctx)
}
pub(super) fn xml_child(
    value: &Value,
    format: &str,
    ctx: &WriterContext,
) -> Result<Vec<Xml>, String> {
    let m = node(value)?;
    if m.class == "Math::Function::None" {
        return Ok(vec![if format == "mathml" {
            el("none", vec![])
        } else {
            run("&#8203;")
        }]);
    }
    if m.class == "Math::Function::Linebreak" {
        return linebreak_xml(&m, format, ctx);
    }
    render_xml(&m, format, ctx)
}
fn source_error(message: &str) -> String {
    format!("source_render_error: Multiscript {message}")
}
fn empty(value: &Value) -> Result<bool, String> {
    match value {
        Value::Array(v) => Ok(v.is_empty()),
        Value::String(v) => Ok(v.is_empty()),
        _ => Err(source_error("field has no source empty? method")),
    }
}
fn mapped(
    value: &Value,
    format: &str,
    nil_allowed: bool,
    ctx: &super::WriterContext,
) -> Result<String, String> {
    if nil_allowed && value.is_null() {
        return Ok(String::new());
    }
    let values = value
        .as_array()
        .ok_or_else(|| source_error("script field has no source map method"))?;
    values
        .iter()
        .map(|v| child(v, format, ctx))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| v.join(""))
}
fn mini(value: &Value) -> Result<bool, String> {
    if value.is_null() {
        return Ok(false);
    }
    if !value.is_object() || value["class"].as_str().is_none() {
        return Err(source_error(
            "filtered script has no source mini_sized? method",
        ));
    }
    let name = value["class"].as_str().unwrap_or("");
    Ok(
        (name == "Math::Number" || name.starts_with("Math::Symbols::"))
            && (truthy(&value["fields"]["mini_sub_sized"])
                || truthy(&value["fields"]["mini_sup_sized"])),
    )
}
fn prime(value: &Value, ctx: &super::WriterContext) -> Result<bool, String> {
    if !value["class"]
        .as_str()
        .is_some_and(|v| v.starts_with("Math::Symbols::"))
    {
        return Ok(false);
    }
    if value["fields"]["value"]
        .as_str()
        .is_some_and(|v| v.contains("&#x27;"))
    {
        return Ok(true);
    }
    let rendered = child(value, "unicodemath", ctx)?;
    Ok(["′", "″", "‴", "⁗"].iter().any(|v| rendered.contains(v)))
}
fn flatten_compact<'a>(value: &'a Value, output: &mut Vec<&'a Value>) {
    if let Some(values) = value.as_array() {
        for value in values {
            flatten_compact(value, output);
        }
    } else if !value.is_null() {
        output.push(value);
    }
}
fn filtered(values: &Value) -> Value {
    if !values.is_array() {
        return values.clone();
    }
    let mut output = vec![];
    flatten_compact(values, &mut output);
    match output.len() {
        0 => Value::Null,
        1 => output[0].clone(),
        _ => serde_json::json!({"class":"Math::Formula","fields":{"value":output}}),
    }
}

pub(super) fn text(m: &Model, format: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if m.class == "Math::Function::Linebreak" {
        return linebreak_text(m, format, ctx);
    }
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    match format {
        "asciimath" | "latex" => {
            // Source valid_value_exist? is !field && (field&.empty? || ...).
            // It is never truthy: nil still emits an empty pair of scripts;
            // false raises through its missing empty? method.
            if b == false || c == false {
                return Err(source_error("false script has no source empty? method"));
            }
            let sub = mapped(b, format, true, ctx)?;
            let sup = mapped(c, format, true, ctx)?;
            let base = if a.is_null() {
                String::new()
            } else {
                child(a, format, ctx)?
            };
            Ok(if format == "asciimath" {
                format!("\\ _({sub})^({sup}){base}")
            } else {
                format!("{{}}_{{{sub}}}^{{{sup}}}{base}")
            })
        }
        "html" => {
            let mut output = String::new();
            // TernaryFunction HTML calls to_html directly on each truthy
            // field, so Array scripts refuse instead of being joined.
            for field in [a, b, c] {
                if truthy(field) {
                    output.push_str(&format!("<i>{}</i>", child(field, format, ctx)?));
                }
            }
            Ok(output)
        }
        "unicodemath" => {
            let has_sub = !empty(b)?;
            let has_sup = !empty(c)?;
            let mut sub = String::new();
            let mut sup = String::new();
            if has_sub {
                let field = filtered(b);
                let text = mapped(b, format, true, ctx)?;
                sub = if mini(&field)? {
                    text
                } else {
                    format!("_({text})")
                };
            }
            if has_sup {
                // This source helper deliberately classifies parameter_two,
                // although it renders parameter_three. Do not swap fields.
                let field = filtered(b);
                let text = mapped(c, format, false, ctx)?;
                sup = if mini(&field)? || prime(&field, ctx)? {
                    text
                } else if field["class"] == "Math::Function::Power" {
                    format!("^{text}")
                } else {
                    format!("^({text})")
                };
            }
            let base = if a.is_null() {
                String::new()
            } else {
                child(a, format, ctx)?
            };
            Ok(format!("{sub}{sup} {base}"))
        }
        _ => Err(format!("unported Multiscript text format {format}")),
    }
}
fn xml_slot(value: &Value, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if format == "omml" && !truthy(value) {
        return Ok(vec![run("&#8203;")]);
    }
    if value.is_null() {
        return Ok(vec![]);
    }
    let values = if let Some(values) = value.as_array() {
        values.as_slice()
    } else {
        std::slice::from_ref(value)
    };
    let mut output = vec![];
    for value in values {
        output.extend(xml_child(value, format, ctx)?);
    }
    Ok(output)
}
fn prescripts(b: &Value, c: &Value) -> Result<Vec<Value>, String> {
    // Source b&.nil? returns nil for nil, not true; nil subscript plus
    // nonempty superscript therefore reaches Array(nil).zip(...) => [].
    if !b.is_null() && empty(b)? {
        let mut output = vec![];
        if c.is_null() {
            return Ok(output);
        }
        if let Some(c) = c.as_array() {
            output.extend(c.iter().cloned());
        } else {
            output.push(c.clone());
        }
        return Ok(output);
    }
    if c.is_null() || empty(c)? {
        return Ok(if b.is_null() {
            vec![]
        } else if let Some(b) = b.as_array() {
            b.clone()
        } else {
            vec![b.clone()]
        });
    }
    let left = if b.is_null() {
        vec![]
    } else if let Some(b) = b.as_array() {
        b.clone()
    } else {
        vec![b.clone()]
    };
    let right = if let Some(c) = c.as_array() {
        c.clone()
    } else {
        vec![c.clone()]
    };
    let mut joined = vec![];
    for (i, l) in left.iter().enumerate() {
        flatten_compact(l, &mut joined);
        if let Some(r) = right.get(i) {
            flatten_compact(r, &mut joined);
        }
    }
    Ok(joined.into_iter().cloned().collect())
}
pub(super) fn xml(m: &Model, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if m.class == "Math::Function::Linebreak" {
        return linebreak_xml(m, format, ctx);
    }
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    if format == "omml" {
        return Ok(vec![el(
            "m:sPre",
            vec![
                el("m:e", xml_slot(a, format, ctx)?),
                el("m:sub", xml_slot(b, format, ctx)?),
                el("m:sup", xml_slot(c, format, ctx)?),
            ],
        )]);
    }
    if format != "mathml" {
        return Err(format!("unported Multiscript XML format {format}"));
    }
    let mut children = vec![];
    if !a.is_null() {
        let base = node(a)?;
        if base.class != "Math::Function::PowerBase" {
            return Err(source_error("base has no source mmultiscript method"));
        }
        for field in ["parameter_one", "parameter_two", "parameter_three"] {
            children.extend(xml_slot(base.field(field), format, ctx)?);
        }
    }
    if truthy(b) || truthy(c) {
        children.push(el("mprescripts", vec![]));
    }
    for value in prescripts(b, c)? {
        // validate_mathml_fields maps each final Array entry directly;
        // nil and nested Arrays here must refuse rather than disappear.
        children.extend(xml_child(&value, format, ctx)?);
    }
    Ok(vec![el("mmultiscripts", children)])
}

/// Plain Linebreak rendering is separate from Formula line splitting. These
/// procedures preserve the leaf's payload/style behavior without rewriting
/// the caller's tree or depending on process-global render context.
fn linebreak_text(m: &Model, format: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let value = m.field("parameter_one");
    if format == "unicodemath" {
        return Ok(format!(
            "&#xa;{}",
            if truthy(value) {
                child(value, format, ctx)?
            } else {
                String::new()
            }
        ));
    }
    let token = match format {
        "asciimath" => "\\\n ",
        "latex" => "\\\\ ",
        "html" => "<br/>",
        _ => return Err(format!("unported Linebreak text format {format}")),
    };
    if !truthy(value) {
        return Ok(token.into());
    }
    let properties = m
        .field("attributes")
        .as_object()
        .ok_or("source_render_error: Linebreak attributes have no [] method")?;
    let after = properties
        .get("linebreakstyle")
        .is_some_and(|v| v == "after");
    let body = if format != "html" && value.is_array() {
        let mut pieces = vec![];
        for v in value.as_array().unwrap() {
            if !v.is_null() {
                pieces.push(child(v, format, ctx)?);
            }
        }
        pieces.join(if format == "latex" { " " } else { "" })
    } else {
        child(value, format, ctx)?
    };
    Ok(if after {
        format!("{body}{token}")
    } else {
        format!("{token}{body}")
    })
}
fn linebreak_xml(m: &Model, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = m.field("parameter_one");
    if format == "omml" {
        return if value.is_null() {
            Ok(vec![])
        } else {
            xml_child(value, format, ctx)
        };
    }
    if format != "mathml" {
        return Err(format!("unported Linebreak XML format {format}"));
    }
    if !truthy(value) {
        return Ok(vec![attrs(
            "mo",
            vec![("linebreak".into(), "newline".into())],
            vec![],
        )]);
    }
    let mut nodes = xml_child(value, format, ctx)?;
    if nodes.len() != 1 {
        return Err("source_render_error: Linebreak payload has no single XML name".into());
    }
    let element = nodes.remove(0);
    let Xml::El(_, mut attributes, children) = element else {
        return Err("source_render_error: Linebreak payload has no XML name".into());
    };
    let properties = m
        .field("attributes")
        .as_object()
        .ok_or("source_render_error: Linebreak attributes have no empty? method")?;
    for (name, value) in properties {
        let string = match value {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            _ => return Err("unported compound Linebreak XML attribute value".into()),
        };
        if let Some((_, existing)) = attributes.iter_mut().find(|(key, _)| key == name) {
            *existing = string;
        } else {
            attributes.push((name.clone(), string));
        }
    }
    Ok(vec![attrs("mo", attributes, children)])
}
