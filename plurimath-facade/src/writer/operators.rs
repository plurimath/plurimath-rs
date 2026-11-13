//! Source PowerBase procedures and the Mod/Sqrt OMML insertion boundaries.
use super::{
    attrs, control_properties, el, render_text, render_xml, run, text, Model, Value, WriterContext,
    Xml,
};
use std::sync::OnceLock;
fn truthy(value: &Value) -> bool {
    !value.is_null() && value != false
}
fn model(value: &Value) -> Result<Model, String> {
    Model::from_value(value)
}
fn child(value: &Value, format: &str, ctx: &super::WriterContext) -> Result<String, String> {
    render_text(&model(value)?, format, ctx)
}
fn tags() -> &'static Value {
    static TABLE: OnceLock<Value> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/operator-writers/source-class-tags.json"
        ))
        .expect("source operator class properties")
    })
}
fn property(value: &Value, omml: bool) -> Result<String, String> {
    if value.is_null() {
        return Ok(if omml { "subSup" } else { "subsup" }.into());
    }
    let node = model(value)
        .map_err(|_| "source_render_error: script base has no source tag-name method".to_owned())?;
    if !omml && node.class == "Math::Function::Nary" {
        let options = node
            .field("options")
            .as_object()
            .ok_or("source_render_error: Nary options are not a hash")?;
        let over = options.get("type").is_some_and(|x| x == "undOvr");
        let sub = truthy(node.field("parameter_two"));
        let sup = truthy(node.field("parameter_three"));
        return Ok(match (sub, sup, over) {
            (true, true, true) => "munderover",
            (true, true, false) => "msubsup",
            (true, false, true) => "munder",
            (true, false, false) => "msub",
            (false, true, true) => "mover",
            (false, true, false) => "msup",
            _ => "mrow",
        }
        .into());
    }
    Ok(
        tags()["classes"][node.class.as_str()][if omml { "omml_tag_name" } else { "tag_name" }]
            .as_str()
            .unwrap_or(if omml { "subSup" } else { "subsup" })
            .into(),
    )
}
fn mini(value: &Value) -> bool {
    let class = value["class"].as_str().unwrap_or("");
    (class == "Math::Number" || class.starts_with("Math::Symbols::"))
        && (truthy(&value["fields"]["mini_sub_sized"])
            || truthy(&value["fields"]["mini_sup_sized"]))
}
fn prime(value: &Value, ctx: &super::WriterContext) -> Result<bool, String> {
    if !value["class"]
        .as_str()
        .unwrap_or("")
        .starts_with("Math::Symbols::")
    {
        return Ok(false);
    }
    if value["fields"]["value"]
        .as_str()
        .is_some_and(|v| v.contains("&#x27;"))
    {
        return Ok(true);
    }
    let glyph = child(value, "unicodemath", ctx)?;
    Ok(["′", "″", "‴", "⁗"].iter().any(|p| glyph.contains(p)))
}
fn unicode_parens(value: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    let content = child(value, "unicodemath", ctx)?;
    Ok(
        if value["class"]
            .as_str()
            .is_some_and(|class| class.starts_with("Math::Function::Fenced"))
        {
            content
        } else {
            format!("({content})")
        },
    )
}
pub(super) fn text_writer(
    node: &Model,
    format: &str,
    ctx: &super::WriterContext,
) -> Result<String, String> {
    if node.class != "Math::Function::PowerBase" {
        return Err("unported operator text class".into());
    }
    let a = node.field("parameter_one");
    let b = node.field("parameter_two");
    let c = node.field("parameter_three");
    match format {
        "asciimath" => {
            let mut out = String::new();
            if truthy(a) {
                let base = child(a, format, ctx)?;
                // Source `when Formula || ...` is a Ruby constant expression:
                // it matches Formula only, rather than every Function class.
                if a["class"].as_str().is_some_and(|class| {
                    class == "Math::Formula" || class.starts_with("Math::Formula::")
                }) {
                    out.push_str(&format!("({base})"))
                } else {
                    out.push_str(&base)
                }
            }
            if truthy(b) {
                out.push_str(&format!("_({})", child(b, format, ctx)?))
            }
            if truthy(c) {
                out.push_str(&format!("^({})", child(c, format, ctx)?))
            }
            Ok(out)
        }
        "latex" => Ok(format!(
            "{}_{{{}}}^{{{}}}",
            if truthy(a) {
                child(a, format, ctx)?
            } else {
                String::new()
            },
            if truthy(b) {
                child(b, format, ctx)?
            } else {
                String::new()
            },
            if truthy(c) {
                child(c, format, ctx)?
            } else {
                String::new()
            }
        )),
        "html" => {
            let mut out = String::new();
            if truthy(a) {
                let base = child(a, format, ctx)?;
                if a["class"] == "Math::Function::FontStyle::Normal" {
                    out.push_str(&base)
                } else {
                    out.push_str(&format!("<i>{base}</i>"))
                }
            }
            if truthy(b) {
                out.push_str(&format!("<sub>{}</sub>", child(b, format, ctx)?))
            }
            if truthy(c) {
                out.push_str(&format!("<sup>{}</sup>", child(c, format, ctx)?))
            }
            Ok(out)
        }
        "unicodemath" => {
            let base = if a.is_null() {
                String::new()
            } else {
                child(a, format, ctx)?
            };
            let sub = if !truthy(b) {
                String::new()
            } else if mini(b) {
                child(b, format, ctx)?
            } else if b["class"] == "Math::Function::Base" {
                format!("_{}", child(b, format, ctx)?)
            } else {
                format!("_{}", unicode_parens(b, ctx)?)
            };
            let is_prime = prime(c, ctx)?;
            let sup = if !truthy(c) {
                String::new()
            } else if mini(c) || is_prime {
                child(c, format, ctx)?
            } else if c["class"] == "Math::Function::Power"
                || (a["class"] == "Math::Function::Power"
                    && prime(&a["fields"]["parameter_two"], ctx)?)
            {
                format!("^{}", child(c, format, ctx)?)
            } else {
                format!("^{}", unicode_parens(c, ctx)?)
            };
            Ok(if is_prime {
                format!("{base}{sup}{sub}")
            } else {
                format!("{base}{sub}{sup}")
            })
        }
        _ => Err(format!("unported operator text format {format}")),
    }
}
fn slot(value: &Value, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
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
    let mut output = Vec::new();
    for value in values {
        output.extend(render_xml(&model(value)?, format, ctx)?)
    }
    Ok(output)
}
fn omml_slot(name: &str, value: &Value, ctx: &WriterContext) -> Result<Xml, String> {
    Ok(el(name, slot(value, "omml", ctx)?))
}
fn script(name: &str, base: Vec<Xml>, argument: Vec<Xml>) -> Xml {
    let (property, argument_name) = match name {
        "m:sSup" => ("m:sSupPr", "m:sup"),
        "m:sSub" => ("m:sSubPr", "m:sub"),
        "m:limUpp" => ("m:limUppPr", "m:lim"),
        _ => ("m:limLowPr", "m:lim"),
    };
    el(
        name,
        vec![
            el(property, vec![control_properties()]),
            el("m:e", base),
            el(argument_name, argument),
        ],
    )
}
/// Parent context propagation can supply an explicit style here. The current
/// seam below refuses ambiguous nondefault under/over contexts.
pub(super) fn xml_with_style(
    node: &Model,
    format: &str,
    ctx: &WriterContext,
    display_style: Option<bool>,
) -> Result<Vec<Xml>, String> {
    let a = node.field("parameter_one");
    let b = node.field("parameter_two");
    let c = node.field("parameter_three");
    match node.class.as_str() {
        "Math::Function::PowerBase" => {
            let tag = property(a, format == "omml")?;
            if format == "mathml" {
                let mut children = slot(a, format, ctx)?;
                children.extend(slot(b, format, ctx)?);
                children.extend(slot(c, format, ctx)?);
                return Ok(vec![el(&format!("m{tag}"), children)]);
            }
            if tag == "undOvr" {
                let style = display_style
                    .ok_or("unported explicit display-style context for under/over PowerBase")?;
                let upper = script(
                    if style { "m:limUpp" } else { "m:sSup" },
                    slot(a, format, ctx)?,
                    slot(c, format, ctx)?,
                );
                return Ok(vec![if truthy(b) {
                    script(
                        if style { "m:limLow" } else { "m:sSub" },
                        vec![upper],
                        slot(b, format, ctx)?,
                    )
                } else {
                    upper
                }]);
            }
            Ok(vec![el(
                "m:sSubSup",
                vec![
                    el("m:sSubSupPr", vec![control_properties()]),
                    omml_slot("m:e", a, ctx)?,
                    omml_slot("m:sub", b, ctx)?,
                    omml_slot("m:sup", c, ctx)?,
                ],
            )])
        }
        "Math::Function::Sqrt" if format == "omml" => Ok(vec![el(
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
                omml_slot("m:e", a, ctx)?,
            ],
        )]),
        "Math::Function::Mod" if format == "omml" => {
            let mut output = vec![];
            if truthy(a) {
                output.extend(render_xml(&model(a)?, format, ctx)?)
            }
            if !truthy(node.field("hide_function_name")) {
                output.push(el(
                    "m:r",
                    vec![
                        el(
                            "m:rPr",
                            vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
                        ),
                        el("m:t", vec![text("mod")]),
                    ],
                ))
            }
            if truthy(b) {
                output.extend(render_xml(&model(b)?, format, ctx)?)
            }
            Ok(output)
        }
        _ => Err(format!("unported operator XML class {}", node.class)),
    }
}
pub(super) fn xml_writer(
    node: &Model,
    format: &str,
    ctx: &WriterContext,
) -> Result<Vec<Xml>, String> {
    xml_with_style(node, format, ctx, Some(ctx.display_style))
}
