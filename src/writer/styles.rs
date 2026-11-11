//! Source color writers; unsupported style options never silently disappear.
use super::{attrs, child, present, strict_slot_xml, Xml};
use crate::model::Model;
fn background(m: &Model) -> Result<bool, String> {
    let options = m.field("options");
    if options.is_null() {
        return Ok(false);
    }
    let map = options
        .as_object()
        .ok_or("source_render_error: color options must support lookup")?;
    if map.keys().any(|key| key != "backgroundcolor") {
        return Err("unported source color options".into());
    }
    Ok(map.get("backgroundcolor").is_some_and(present))
}
fn color(m: &Model) -> Result<String, String> {
    Ok(child(m.field("parameter_one"), "asciimath")?
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect())
}
pub(super) fn text(m: &Model, f: &str) -> Result<String, String> {
    let bg = background(m)?;
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    Ok(match f {
        "asciimath" => format!(
            "color({})({})",
            if present(a) { color(m)? } else { String::new() },
            if present(b) {
                child(b, f)?
            } else {
                String::new()
            }
        ),
        "latex" => format!(
            "{{\\color{{{}}} {}}}",
            if present(a) { color(m)? } else { String::new() },
            if present(b) {
                child(b, f)?
            } else {
                String::new()
            }
        ),
        "html" => format!(
            "{}{}",
            if present(a) {
                format!("<i>{}</i>", child(a, f)?)
            } else {
                String::new()
            },
            if present(b) {
                format!("<i>{}</i>", child(b, f)?)
            } else {
                String::new()
            }
        ),
        "unicodemath" => {
            if a.is_null() || b.is_null() {
                return Err(
                    "source_render_error: color arguments must receive to_unicodemath".into(),
                );
            }
            format!(
                "{}({}&{})",
                if bg { "☁" } else { "✎" },
                child(a, f)?,
                child(b, f)?
            )
        }
        _ => return Err("unknown source color text format".into()),
    })
}
pub(super) fn xml(m: &Model, f: &str, spacing: bool, templates: bool) -> Result<Vec<Xml>, String> {
    let bg = background(m)?;
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    if f == "omml" {
        if b.is_null() {
            return Err("source_render_error: color expression cannot receive insert_t_tag".into());
        }
        return strict_slot_xml(b, f, spacing, templates, false);
    }
    let attributes = if present(a) {
        vec![(
            if bg { "mathbackground" } else { "mathcolor" }.into(),
            color(m)?.replace('"', ""),
        )]
    } else {
        vec![]
    };
    Ok(vec![attrs(
        "mstyle",
        attributes,
        strict_slot_xml(b, f, spacing, templates, false)?,
    )])
}
