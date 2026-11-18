//! Source Overleftrightarrow and Semantics procedures (Menclose lives in `enclose`).
use super::{
    attrs, child, el, omml_slot, overset_xml, parens, render_xml, run, strict_slot_xml, text,
    Model, Value, WriterContext, Xml,
};
use serde_json::json;

pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Overleftrightarrow" | "Math::Function::Semantics"
    )
}

pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    match m.kind() {
        "Overleftrightarrow" => overleftrightarrow_text(m, f, ctx),
        _ => semantics_text(m, f, ctx),
    }
}

pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    match m.kind() {
        "Overleftrightarrow" => overleftrightarrow_xml(m, f, ctx),
        _ => semantics_xml(m, f, ctx),
    }
}

/// The node argument; arrays and scalars are not source node shapes here.
fn node_slot<'a>(m: &'a Model, key: &str) -> Result<&'a Value, String> {
    let v = m.field(key);
    if v.is_null() || v.is_object() {
        Ok(v)
    } else {
        Err(format!("unported promised {} {key} shape", m.class))
    }
}

fn overleftrightarrow_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let value = node_slot(m, "parameter_one")?;
    let present = !value.is_null();
    match f {
        // "overleftrightarrow" is not a UNARY_CLASSES keyword.
        "asciimath" => Ok(if present {
            format!("overleftrightarrow({})", child(value, f, ctx)?)
        } else {
            "overleftrightarrow".into()
        }),
        "latex" => Ok(format!("\\overleftrightarrow{{{}}}", child(value, f, ctx)?)),
        "html" => Ok(format!(
            "<i>&#x20e1;</i>{}",
            if present {
                format!("<i>{}</i>", child(value, f, ctx)?)
            } else {
                String::new()
            }
        )),
        "unicodemath" => Ok(format!("{}\u{20e1}", parens(value, f, ctx)?)),
        _ => Err(format!("unknown Overleftrightarrow text format {f}")),
    }
}

fn accent(m: &Model) -> Result<Option<String>, String> {
    match m.field("attributes") {
        Value::Null => Ok(None),
        Value::Object(values) => match values.get("accent") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(Value::Bool(true)) => Ok(Some("true".into())),
            Some(_) => Err("unported promised Overleftrightarrow accent value".into()),
        },
        _ => Err("unported promised Overleftrightarrow attributes shape".into()),
    }
}

fn overleftrightarrow_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = node_slot(m, "parameter_one")?;
    let accent = accent(m)?;
    if f == "mathml" {
        let mut content = strict_slot_xml(value, f, ctx, false)?;
        content.push(el("mo", vec![text("&#x20e1;")]));
        let a = accent
            .map(|v| vec![("accent".into(), v)])
            .unwrap_or_default();
        return Ok(vec![attrs("mover", a, content)]);
    }
    if value.is_null() {
        return Ok(vec![run("&#x20e1;")]);
    }
    if accent.is_some() {
        let props = el(
            "m:accPr",
            vec![attrs(
                "m:chr",
                vec![("m:val".into(), "\u{20e1}".into())],
                vec![],
            )],
        );
        return Ok(vec![el(
            "m:acc",
            vec![props, el("m:e", omml_slot(value, f, ctx)?)],
        )]);
    }
    // Overset.new(Symbol.new("⃡"), parameter_one).to_omml_without_math_tag(true)
    let overset = crate::model::node(
        "Math::Function::Overset",
        json!({
            "parameter_one": crate::model::node("Math::Symbols::Symbol", json!({"value": "\u{20e1}"})),
            "parameter_two": value,
        }),
    );
    overset_xml(
        &Model::from_value(&overset)?,
        f,
        &ctx.with_display_style(true),
    )
}

fn semantics_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    match f {
        "asciimath" | "latex" => child(node_slot(m, "parameter_one")?, f, ctx),
        // BinaryFunction defaults with the annotation array are not ported.
        _ => Err(format!("unported promised Semantics {f} writer")),
    }
}

fn semantics_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = node_slot(m, "parameter_one")?;
    if f == "omml" {
        if value.is_null() {
            return Err(
                "source_no_method_error: Semantics parameter_one nil has no insert_t_tag".into(),
            );
        }
        return render_xml(&Model::from_value(value)?, f, ctx);
    }
    let mut content = strict_slot_xml(value, f, ctx, false)?;
    match m.field("parameter_two") {
        Value::Null => {}
        Value::Array(entries) => {
            for entry in entries {
                let Some(tags) = entry.as_object() else {
                    return Err("unported promised Semantics annotation entry".into());
                };
                for (tag, values) in tags {
                    let mut nodes = vec![];
                    match values {
                        Value::Null => {}
                        Value::Array(values) => {
                            for v in values {
                                nodes.extend(strict_slot_xml(v, f, ctx, false)?);
                            }
                        }
                        _ => return Err("unported promised Semantics annotation content".into()),
                    }
                    content.push(el(tag, nodes));
                }
            }
        }
        _ => return Err("unported promised Semantics annotation shape".into()),
    }
    Ok(vec![el("semantics", content)])
}
