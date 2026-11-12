//! Source Base subscript writers. Options outside the implemented profile fail explicitly.
use super::WriterContext;
use super::{child, control_properties, el, present, run, strict_slot_xml, Xml};
use crate::model::Model;
fn options(m: &Model) -> Result<(), String> {
    let v = m.field("options");
    if !v.is_null() && v.as_object().is_none_or(|x| !x.is_empty()) {
        return Err("unported source subscript size/options".into());
    }
    Ok(())
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    options(m)?;
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let first = if present(a) {
        child(a, f, ctx)?
    } else {
        String::new()
    };
    Ok(match f {
        "asciimath" => format!(
            "{first}{}",
            if present(b) {
                format!("_({})", child(b, f, ctx)?)
            } else {
                String::new()
            }
        ),
        "latex" => format!(
            "{}_{{{}}}",
            if a["class"]
                .as_str()
                .is_some_and(|c| c == "Math::Formula" || c.starts_with("Math::Formula::"))
            {
                format!("{{{first}}}")
            } else {
                first
            },
            if present(b) {
                child(b, f, ctx)?
            } else {
                String::new()
            }
        ),
        "html" => format!(
            "{}{}",
            if present(a) {
                format!("<i>{first}</i>")
            } else {
                String::new()
            },
            if present(b) {
                format!("<sub>{}</sub>", child(b, f, ctx)?)
            } else {
                String::new()
            }
        ),
        "unicodemath" => {
            if b["fields"]["mini_sub_sized"] == true || b["fields"]["mini_sup_sized"] == true {
                return Err("unported source mini-sized subscript".into());
            }
            let suffix = if b.is_null() {
                "()".into()
            } else if b["class"] == "Math::Function::Base" || b["class"] == "Math::Function::Fenced"
            {
                format!("_{}", child(b, f, ctx)?)
            } else {
                format!("_({})", child(b, f, ctx)?)
            };
            format!("{first}{suffix}")
        }
        _ => return Err("unknown subscript text format".into()),
    })
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    options(m)?;
    if f == "mathml"
        && [m.field("parameter_one"), m.field("parameter_two")]
            .iter()
            .any(|value| value.is_array())
    {
        return Err("source_render_error: array cannot receive to_mathml_without_math_tag".into());
    }
    let slot = |name: &str| -> Result<Vec<Xml>, String> {
        let value = m.field(name);
        if value
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.is_null()))
        {
            return Err("source_render_error: subscript array includes nil node".into());
        }
        if f == "omml" && !present(value) {
            Ok(vec![run("&#8203;")])
        } else {
            strict_slot_xml(value, f, ctx, true)
        }
    };
    let a = slot("parameter_one")?;
    let b = slot("parameter_two")?;
    if f == "mathml" {
        let under = [
            "Math::Function::Ubrace",
            "Math::Function::Obrace",
            "Math::Function::Right",
            "Math::Function::Max",
            "Math::Function::Min",
        ]
        .iter()
        .any(|x| m.field("parameter_one")["class"] == *x);
        let mut c = a;
        c.extend(b);
        return Ok(vec![el(if under { "munder" } else { "msub" }, c)]);
    }
    Ok(vec![el(
        "m:sSub",
        vec![
            el("m:sSubPr", vec![control_properties()]),
            el("m:e", a),
            el("m:sub", b),
        ],
    )])
}
