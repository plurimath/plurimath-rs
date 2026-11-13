//! Explicit Intent source procedures; metadata is content only in the source text writers.
use super::{Model, Value, WriterContext, Xml};
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn child(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    super::multiscripts::child(v, f, ctx)
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let is_arg = m.class == "Math::Function::Arg";
    let first = m.field("parameter_one");
    let second = m.field("parameter_two");
    match f {
        "asciimath" => {
            let mut result = if is_arg { "arg" } else { "intent" }.to_owned();
            for field in [first, second] {
                if truthy(field) {
                    result.push_str(&format!("({})", child(field, f, ctx)?));
                }
            }
            Ok(result)
        }
        "latex" => {
            let mut result = if is_arg { "\\arg" } else { "\\intent" }.to_owned();
            for field in [first, second] {
                if truthy(field) {
                    result.push_str(&super::nary::latex_wrapped_with_value(
                        field,
                        child(field, "latex", ctx)?,
                    )?);
                }
            }
            Ok(result)
        }
        "html" => {
            let mut out = String::new();
            for value in if is_arg {
                vec![first, second]
            } else {
                vec![first]
            } {
                if truthy(value) {
                    out.push_str(&format!("<i>{}</i>", child(value, f, ctx)?));
                }
            }
            Ok(out)
        }
        "unicodemath" => {
            if !truthy(first) && !truthy(second) {
                return Ok(if is_arg { "ⓐ" } else { "ⓘ" }.into());
            }
            if is_arg {
                let first = if first.is_null() {
                    String::new()
                } else {
                    child(first, f, ctx)?
                };
                let second = if second.is_null() {
                    String::new()
                } else {
                    child(second, f, ctx)?
                };
                Ok(format!("ⓐ({second} {first})"))
            } else {
                Ok(format!(
                    "ⓘ({}{})",
                    child(second, f, ctx)?,
                    child(first, f, ctx)?
                ))
            }
        }
        _ => Err("unported ExplicitIntent text format".into()),
    }
}
fn metadata(value: &Value) -> Result<(String, bool), String> {
    let model = Model::from_value(value)?;
    let value = if model.kind() == "Text" {
        model.field("parameter_one")
    } else {
        model
            .fields
            .get("value")
            .ok_or("source_render_error: intent metadata has no value accessor")?
    };
    if let Some(text) = value.as_str() {
        return Ok((super::text_content::decode(text)?, text == ":derivative"));
    }
    if value.is_number() {
        return Ok((value.to_string(), false));
    }
    Err("unported promised Ruby non-string intent metadata inspection".into())
}
fn derivative(m: &Model, ctx: &super::WriterContext) -> Result<Option<String>, String> {
    let first = Model::from_value(m.field("parameter_one"))?;
    if first.class != "Math::Formula" {
        return Ok(None);
    }
    let Some(power) = first.field("value").as_array().and_then(|v| v.first()) else {
        return Ok(None);
    };
    let power = Model::from_value(power)?;
    if power.kind() != "Power" || !super::prime_symbol(power.field("parameter_two"), ctx)? {
        return Ok(None);
    }
    let base = Model::from_value(power.field("parameter_one"))?;
    if !base.class.starts_with("Math::Symbols::") {
        return Ok(None);
    }
    Ok(Some(super::text_content::decode(
        base.field("value")
            .as_str()
            .ok_or("source_render_error: derivative symbol has no String value")?,
    )?))
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = m.field("parameter_one");
    let is_arg = m.class == "Math::Function::Arg";
    if f == "omml" {
        if is_arg {
            let mut body = vec![];
            for value in [value, m.field("parameter_two")] {
                if truthy(value) {
                    body.extend(super::multiscripts::xml_child(value, f, ctx)?);
                }
            }
            return Ok(vec![super::el("m:r", body)]);
        }
        return if truthy(value) {
            super::multiscripts::xml_child(value, f, ctx)
        } else {
            Ok(vec![])
        };
    }
    let first = Model::from_value(value)?;
    if first.class == "Math::Formula" && !truthy(first.field("left_right_wrapper")) {
        return Err("source_render_error: intent child returned an Array".into());
    }
    let mut nodes = super::multiscripts::xml_child(value, f, ctx)?;
    if nodes.len() != 1 || !matches!(nodes[0], Xml::El(..)) {
        return Err("source_render_error: intent child has no element attributes".into());
    }
    let (mut annotation, derivative_requested) = metadata(m.field("parameter_two"))?;
    if !is_arg && derivative_requested {
        if let Some(base) = derivative(m, ctx)? {
            let children = super::xml_nodes(&nodes[0]);
            let fence = children
                .get(1)
                .ok_or("source_render_error: derivative has no second XML child")?;
            if !matches!(fence,Xml::El(_,attrs,_) if attrs.iter().any(|(key,value)|key=="intent"&&value==":fenced"))
            {
                return Err("source_render_error: derivative fence metadata is absent".into());
            }
            let content = super::xml_nodes(fence);
            let mut unfenced = String::new();
            for node in content.iter().skip(1).take(content.len().saturating_sub(2)) {
                if super::xml_name(node) != "mi" {
                    break;
                }
                unfenced.push_str(&super::text_content::decode(super::xml_first_text(node))?);
            }
            let fenced = if unfenced.is_empty() {
                String::new()
            } else {
                format!("({unfenced})")
            };
            annotation = format!(":derivative(1,{base}{fenced},{unfenced})");
        }
    }
    super::xml_attribute(
        &mut nodes[0],
        if is_arg { "arg" } else { "intent" },
        annotation,
    );
    Ok(nodes)
}
