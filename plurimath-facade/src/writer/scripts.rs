//! Source Base subscript writers. Options outside the implemented profile fail explicitly.
use super::WriterContext;
use super::{child, control_properties, el, present, run, strict_slot_xml, Xml};
use crate::model::Model;
/// Ruby `options` of Base: `nil` or a Hash; only `options[:size]` affects UnicodeMath output.
fn options(m: &Model) -> Result<crate::source_values::RubyValue, String> {
    use crate::source_values::{field, Domain, RubyValue};
    let v = field(m, "options", Domain::NamedOptions)?;
    match v {
        RubyValue::Nil | RubyValue::Hash(_) => Ok(v),
        _ => Err("unported source subscript options shape (not nil or Hash)".into()),
    }
}
fn options_present(o: &crate::source_values::RubyValue) -> bool {
    use crate::source_values::RubyValue;
    matches!(o, RubyValue::Hash(e) if !e.is_empty())
}
/// `size_overrides`: nil without a :size, else "Ⅎ" + the inverted SIZE_OVERRIDES_SYMBOLS key.
fn size_overrides(o: &crate::source_values::RubyValue) -> Result<String, String> {
    use crate::source_values::RubyValue;
    match o.get_symbol("size") {
        None | Some(RubyValue::Nil) | Some(RubyValue::Bool(false)) => Ok(String::new()),
        Some(RubyValue::String(v)) => Ok(format!(
            "Ⅎ{}",
            match v.as_str() {
                "1.25em" => "A",
                "1.5625em" => "B",
                "0.8em" => "C",
                "0.64em" => "D",
                _ => "",
            }
        )),
        _ => Err("unported source subscript size value shape".into()),
    }
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let opts = options(m)?;
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
            let suffix = if b["class"] == "Math::Function::Base" {
                format!("_{}{}", size_overrides(&opts)?, child(b, f, ctx)?)
            } else if super::numbers::mini_sized(b) {
                child(b, f, ctx)?
            } else if b.is_null() {
                "()".into()
            } else if options_present(&opts) {
                format!("_{}〖{}〗", size_overrides(&opts)?, child(b, f, ctx)?)
            } else if b["class"] == "Math::Function::Fenced" {
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
