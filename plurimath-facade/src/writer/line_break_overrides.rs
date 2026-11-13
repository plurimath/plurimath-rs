//! Source Overset and Mod continuation procedures (Ruby 0a1df68a).
use super::{el, strict_slot_xml, text, WriterContext, Xml};
use super::{filter_source_values, new_source_node, source_break_child};
use crate::model::Model;
use serde_json::{json, Value};

fn present(value: &Value) -> bool {
    !value.is_null() && value != false
}

/// Mod's MathML is an ordered row, including an empty mi when hidden.
/// OMML retains the existing source operator renderer; only receiver admission
/// is applied here so invalid Array slots preserve the source exception class.
pub(super) fn mod_xml(
    model: &Model,
    format: &str,
    ctx: &WriterContext,
) -> Result<Vec<Xml>, String> {
    let mut slots = vec![];
    for key in ["parameter_one", "parameter_two"] {
        let value = model.field(key);
        if present(value) && !value.is_object() {
            return Err("source_render_error: Mod operand has no source XML node writer".into());
        }
        slots.push(if format == "mathml" && present(value) {
            strict_slot_xml(value, format, ctx, false)?
        } else {
            vec![]
        });
    }
    if format == "omml" {
        return super::operators::xml_writer(model, format, ctx);
    }
    let mut content = slots.remove(0);
    content.push(el(
        "mi",
        if present(model.field("hide_function_name")) {
            vec![]
        } else {
            vec![text("mod")]
        },
    ));
    content.extend(slots.remove(0));
    Ok(vec![el("mrow", content)])
}

pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Overset" | "Math::Function::Mod"
    )
}

pub(super) fn line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    let model = Model::from_value(node)?;
    if model.kind() == "Overset" {
        // Overset's annotation is never traversed by this override.
        source_break_child(&mut node["fields"]["parameter_two"], obj, true)?;
        if !obj.is_empty() {
            *obj = vec![new_source_node(
                &model.class,
                json!({
                    "parameter_one":node["fields"]["parameter_one"],
                    "parameter_two":filter_source_values(std::mem::take(obj))
                }),
            )];
            node["fields"]["parameter_one"] = Value::Null;
        }
        return Ok(());
    }
    source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
    if !obj.is_empty() {
        let mut next = new_source_node(
            &model.class,
            json!({
                "parameter_one":filter_source_values(std::mem::take(obj)),
                "parameter_two":node["fields"]["parameter_two"]
            }),
        );
        next["fields"]["hide_function_name"] = Value::Bool(false);
        node["fields"]["hide_function_name"] = Value::Bool(true);
        node["fields"]["parameter_two"] = Value::Null;
        *obj = vec![next];
        return Ok(());
    }
    source_break_child(&mut node["fields"]["parameter_two"], obj, true)?;
    if !obj.is_empty() {
        *obj = vec![new_source_node(
            &model.class,
            json!({
                "parameter_one":null,
                "parameter_two":filter_source_values(std::mem::take(obj)),
                "hide_function_name":true
            }),
        )];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_continuation_state_and_pending_objects() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../fixtures/line-break-overrides/independent-reference.json"
        ))
        .unwrap();
        for row in reference["states"].as_array().unwrap() {
            let mut node = row["model"].clone();
            let mut pending = row["pending"].as_array().unwrap().clone();
            let result = super::super::source_line_breaking(&mut node, &mut pending);
            if row["expected"].get("error").is_some() {
                assert!(result.is_err(), "{}", row["id"])
            } else {
                result.unwrap();
                assert_eq!(node, row["expected"]["node"], "node {}", row["id"]);
                assert_eq!(
                    pending,
                    row["expected"]["continuation"].as_array().unwrap().clone(),
                    "pending {}",
                    row["id"]
                );
            }
        }
    }
}
