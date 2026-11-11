//! Exact Mod and Log source procedures (Ruby 0a1df68a).
use super::{attrs, control_properties, el, run, text, Model, Value, WriterContext, Xml};
fn truthy(value: &Value) -> bool {
    !value.is_null() && value != false
}
fn child(value: &Value, format: &str) -> Result<String, String> {
    super::multiscripts::child(value, format)
}
fn safe(value: &Value, format: &str) -> Result<String, String> {
    if value.is_null() {
        Ok(String::new())
    } else {
        child(value, format)
    }
}
fn parens(value: &Value) -> Result<String, String> {
    let output = child(value, "unicodemath")?;
    Ok(if value["class"] == "Math::Function::Fenced" {
        output
    } else {
        format!("({output})")
    })
}
fn mini(value: &Value) -> Result<bool, String> {
    let model = Model::from_value(value)?;
    if model.class == "Math::Number" || model.class.starts_with("Math::Symbols::") {
        return Ok(truthy(model.field("mini_sub_sized")) || truthy(model.field("mini_sup_sized")));
    }
    if model.class == "Math::Formula" {
        let field = model.field("value");
        if field.is_null() {
            return Ok(false);
        }
        let values = field
            .as_array()
            .ok_or("source_render_error: Formula mini predicate has no first method")?;
        return match values.first() {
            Some(value) if !value.is_null() => mini(value),
            _ => Ok(false),
        };
    }
    if model.class == "Math::Function::Fenced" {
        for field in [
            model.field("parameter_one"),
            model
                .field("parameter_two")
                .as_array()
                .and_then(|v| v.first())
                .unwrap_or(&Value::Null),
            model.field("parameter_three"),
        ] {
            if !field.is_null() && mini(field)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Mod" | "Math::Function::Log"
    )
}
pub(super) fn text_writer(m: &Model, f: &str) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    if m.class == "Math::Function::Mod" {
        return match f {
            "asciimath" => Ok(format!("{} mod {}", safe(a, f)?, safe(b, f)?)),
            "latex" => Ok(format!(
                "{} \\mod {}",
                if truthy(a) {
                    format!("{{{}}}", child(a, f)?)
                } else {
                    String::new()
                },
                if truthy(b) {
                    format!("{{{}}}", child(b, f)?)
                } else {
                    String::new()
                }
            )),
            "html" => Ok(format!(
                "{}<i>mod</i>{}",
                if truthy(a) {
                    format!("<i>{}</i>", child(a, f)?)
                } else {
                    String::new()
                },
                if truthy(b) {
                    format!("<i>{}</i>", child(b, f)?)
                } else {
                    String::new()
                }
            )),
            "unicodemath" => Ok(format!("{}mod{}", safe(a, f)?, safe(b, f)?)),
            _ => Err("unported Mod text format".into()),
        };
    }
    let mut output = match f {
        "asciimath" | "unicodemath" => "log".to_string(),
        "latex" => "\\log".to_string(),
        "html" => "<i>log</i>".to_string(),
        _ => return Err("unported Log text format".into()),
    };
    for (value, upper) in [(a, false), (b, true)] {
        if !truthy(value) {
            continue;
        }
        let marker = if upper { "^" } else { "_" };
        match f {
            "asciimath" => output.push_str(&format!("{marker}({})", child(value, f)?)),
            "latex" => output.push_str(&format!(
                "{marker}{}",
                super::nary::latex_wrapped_with_value(value, child(value, f)?)?
            )),
            "html" => {
                let tag = if upper { "sup" } else { "sub" };
                output.push_str(&format!("<{tag}>{}</{tag}>", child(value, f)?));
            }
            "unicodemath" => {
                if mini(value)? || (upper && super::prime_symbol(value)?) {
                    output.push_str(&child(value, f)?);
                } else if value["class"]
                    == if upper {
                        "Math::Function::Power"
                    } else {
                        "Math::Function::Base"
                    }
                {
                    output.push_str(&format!("{marker}{}", child(value, f)?));
                } else {
                    output.push_str(&format!("{marker}{}", parens(value)?));
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(output)
}
/// Log's source XML is independent of the already qualified Mod XML dispatch.
pub(super) fn log_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let any = truthy(a) || truthy(b);
    let hidden = truthy(m.field("hide_function_name"));
    if f == "mathml" {
        let first = el("mi", if hidden { vec![] } else { vec![text("log")] });
        if !any {
            return Ok(vec![first]);
        }
        let tag = if truthy(a) && truthy(b) {
            "msubsup"
        } else if truthy(a) {
            "msub"
        } else {
            "msup"
        };
        let mut body = vec![first];
        body.extend(super::generic_nary::slot(a, f, ctx)?);
        body.extend(super::generic_nary::slot(b, f, ctx)?);
        return Ok(vec![el(tag, body)]);
    }
    if !any {
        return Ok(vec![run("log")]);
    }
    let base = if hidden {
        vec![]
    } else {
        vec![el(
            "m:r",
            vec![
                el(
                    "m:rPr",
                    vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
                ),
                el("m:t", vec![text("log")]),
            ],
        )]
    };
    Ok(vec![el(
        "m:sSubSup",
        vec![
            el("m:sSubSupPr", vec![control_properties()]),
            el("m:e", base),
            el("m:sub", super::generic_nary::slot(a, f, ctx)?),
            el("m:sup", super::generic_nary::slot(b, f, ctx)?),
        ],
    )])
}
pub(super) fn log_line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    super::source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
    if !obj.is_empty() {
        let next = super::new_source_node(
            "Math::Function::Log",
            serde_json::json!({"parameter_one":super::filter_source_values(std::mem::take(obj)),"parameter_two":node["fields"]["parameter_two"],"hide_function_name":true}),
        );
        node["fields"]["parameter_two"] = Value::Null;
        *obj = vec![next];
    }
    Ok(())
}
