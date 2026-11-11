//! Source-backed FontStyle procedures. Normal remains owned by the existing unit-aware route.
use super::WriterContext;
use super::{attrs, child, el, strict_slot_xml, text as xml_text, value_text, Model, Xml};
use serde_json::Value;
use std::sync::OnceLock;

fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("font_styles-data.json"))
            .expect("sealed source font metadata")
    })
}
pub(super) fn handles(m: &Model) -> bool {
    m.class == "Math::Function::FontStyle"
        || (m.class != "Math::Function::FontStyle::Normal"
            && data()["classes"].get(&m.class).is_some())
}
fn entry(m: &Model) -> Option<&'static Value> {
    data()["classes"].get(&m.class)
}
fn argument(m: &Model) -> Result<&Value, String> {
    let v = m.field("parameter_one");
    if !v.is_null() && !v.is_object() {
        return Err(
            "source_render_error: FontStyle argument cannot receive a source node writer".into(),
        );
    }
    Ok(v)
}
fn family(m: &Model) -> Result<(String, String), String> {
    if let Some(v) = entry(m) {
        return Ok((
            v["mathvariant"].as_str().unwrap_or("").into(),
            v["unicode_family"].as_str().unwrap_or("").into(),
        ));
    }
    let token = m
        .field("parameter_two")
        .as_str()
        .ok_or("source_render_error: FontStyle family cannot receive to_sym")?;
    for value in data()["classes"].as_object().unwrap().values() {
        if value["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == token)
        {
            return Ok((
                value["mathvariant"].as_str().unwrap_or("").into(),
                value["unicode_family"].as_str().unwrap_or("").into(),
            ));
        }
    }
    Ok((token.into(), String::new()))
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let p = argument(m)?;
    if m.class == "Math::Function::FontStyle" {
        let prefix = if f == "unicodemath" {
            family(m)?.1
        } else {
            String::new()
        };
        return Ok(format!("{prefix}{}", child(p, f, ctx)?));
    }
    let info = entry(m).ok_or("unported source FontStyle class")?;
    let parts = &info["text"][f];
    let prefix = parts["prefix"]
        .as_str()
        .ok_or("unported FontStyle text format")?;
    let suffix = parts["suffix"].as_str().unwrap_or("");
    if f == "unicodemath" && m.class == "Math::Function::FontStyle::Monospace" {
        if p.is_null() {
            return Err(
                "source_render_error: Monospace argument cannot receive to_unicodemath".into(),
            );
        }
        if p["class"] == "Math::Function::Fenced" {
            return Ok(format!(
                "{}{}",
                prefix.strip_suffix('(').unwrap_or(prefix),
                child(p, f, ctx)?
            ));
        }
    }
    Ok(format!("{prefix}{}{suffix}", child(p, f, ctx)?))
}
fn properties(m: &Model) -> Vec<Xml> {
    if m.class == "Math::Function::FontStyle" {
        return vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])];
    }
    entry(m).unwrap()["omml_properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            attrs(
                v[0].as_str().unwrap(),
                vec![("m:val".into(), v[1].as_str().unwrap().into())],
                vec![],
            )
        })
        .collect()
}
fn styled_run(m: &Model, children: Vec<Xml>) -> Xml {
    let mut result = vec![el("m:rPr", properties(m))];
    result.extend(children);
    el("m:r", result)
}
fn inject_run(m: &Model, node: &mut Xml) {
    if let Xml::El(_, _, children) = node {
        if !children
            .iter()
            .any(|n| matches!(n,Xml::El(name,_,_)if name=="m:rPr"))
        {
            children.insert(0, el("m:rPr", properties(m)));
        }
    }
}
fn inject_descendants(m: &Model, node: &mut Xml) {
    if let Xml::El(_, _, children) = node {
        for child in children {
            if matches!(child,Xml::El(name,_,_)if name=="m:r") {
                inject_run(m, child)
            } else {
                inject_descendants(m, child)
            }
        }
    }
}
fn font_children(v: &Value, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if v.is_null() {
        return Ok(vec![]);
    }
    let m = Model::from_value(v)?;
    if m.kind() == "Number" {
        return Ok(vec![el(
            "m:t",
            vec![xml_text(&if let Some(formatter)=ctx.formatter {formatter.text(&m,"plain").map_err(|error|super::formatter_context::scope_error(ctx,error))?} else {value_text(m.field("value"))?})],
        )]);
    }
    if m.class.starts_with("Math::Symbols::") {
        let value = m.field("value");
        let text = if !value.is_null() {
            Some(value_text(value)?)
        } else if m.kind() == "Symbol" {
            None
        } else {
            Some(super::symbol(&m, "omml")?)
        };
        return Ok(vec![el(
            "m:t",
            text.map(|s| vec![xml_text(&s)]).unwrap_or_default(),
        )]);
    }
    if ["Text", "Mbox"].contains(&m.kind()) {
        return super::text_content::direct_omml(&m);
    }
    strict_slot_xml(v, "omml", ctx, false)
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let p = argument(m)?;
    if f == "mathml" {
        let children = strict_slot_xml(p, f, ctx, false)?;
        return Ok(vec![attrs(
            "mstyle",
            vec![("mathvariant".into(), family(m)?.0)],
            children,
        )]);
    }
    if f != "omml" {
        return Err("unported FontStyle XML format".into());
    }
    let mut children = font_children(p, ctx)?;
    if children.is_empty() {
        return Ok(vec![styled_run(m, vec![])]);
    }
    if children
        .iter()
        .all(|n| matches!(n, Xml::Text(_)) || matches!(n,Xml::El(name,_,_)if name=="m:t"))
    {
        return Ok(vec![styled_run(m, children)]);
    }
    let mut output = vec![];
    for mut node in children.drain(..) {
        if matches!(&node,Xml::El(name,_,_)if name=="m:r") {
            inject_run(m, &mut node);
            output.push(node);
        } else if matches!(&node,Xml::El(name,_,_)if name=="m:t") {
            output.push(styled_run(m, vec![node]));
        } else {
            inject_descendants(m, &mut node);
            output.push(node);
        }
    }
    Ok(output)
}
