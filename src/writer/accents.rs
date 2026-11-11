//! Procedural ports of the source accent classes, including their distinct nil behavior.
use super::{
    attrs, child, control_properties, el, present, run, strict_slot_xml, text as xml_text,
    unary_slot_text, unary_text, Xml,
};
use crate::model::Model;
use serde_json::Value;

pub(super) fn handles(m: &Model) -> bool {
    m.class.starts_with("Math::Function::")
        && ["Bar", "Hat", "Vec", "Dot", "Ddot", "Tilde", "Ubrace"].contains(&m.kind())
}

fn direct(v: &Value) -> Result<(), String> {
    if present(v) && !v.is_object() {
        return Err("source_render_error: accent argument must receive node methods".into());
    }
    Ok(())
}

fn array_nodes(v: &Value) -> Result<(), String> {
    if v.as_array()
        .is_some_and(|a| a.iter().any(|x| !x.is_object()))
    {
        return Err("source_render_error: accent array contains a non-node".into());
    }
    Ok(())
}

pub(super) fn text(m: &Model, f: &str) -> Result<String, String> {
    let v = m.field("parameter_one");
    if f == "latex" && v == false && !["Bar", "Hat", "Ubrace"].contains(&m.kind()) {
        return Err("source_render_error: false cannot receive to_latex".into());
    }
    if f == "unicodemath" {
        direct(v)?;
        let k = m.kind();
        if !present(v) && !["Dot", "Ubrace"].contains(&k) {
            return Err(
                "source_render_error: accent Unicode argument cannot receive to_unicodemath".into(),
            );
        }
        let body = if present(v) {
            let s = child(v, f)?;
            if k != "Ubrace" && v["class"] == "Math::Function::Fenced" {
                s
            } else {
                format!("({s})")
            }
        } else {
            String::new()
        };
        let glyph = match k {
            "Bar" => "̅",
            "Hat" => "̂",
            "Vec" => "⃗",
            "Dot" => "̇",
            "Ddot" => "̈",
            "Tilde" => "̃",
            "Ubrace" => "⏟",
            _ => unreachable!(),
        };
        return Ok(if k == "Ubrace" {
            format!("{glyph}{body}")
        } else {
            format!("{body}{glyph}")
        });
    }
    if ["Bar", "Hat", "Ubrace"].contains(&m.kind()) && ["asciimath", "latex"].contains(&f) {
        direct(v)?;
        let (name, left, right) = if f == "asciimath" {
            (m.kind().to_lowercase(), "(", ")")
        } else {
            (
                format!(
                    "\\{}",
                    if m.kind() == "Bar" {
                        "overline"
                    } else if m.kind() == "Hat" {
                        "hat"
                    } else {
                        "underbrace"
                    }
                ),
                "{",
                "}",
            )
        };
        return Ok(if present(v) {
            format!("{name}{left}{}{right}", child(v, f)?)
        } else {
            name
        });
    }
    if m.kind() == "Ddot" && f == "html" {
        direct(v)?;
        return Ok(format!(
            "{}<i>..</i>",
            if present(v) {
                format!("<i>{}</i>", child(v, f)?)
            } else {
                String::new()
            }
        ));
    }
    if f == "html" {
        array_nodes(v)?;
        // Mathml::Constants::UNICODE_SYMBOLS.invert[class_name], as used by Core.
        let name = match m.kind() {
            "Bar" => "¯",
            "Hat" => "^",
            "Vec" => "&#x2192;",
            "Dot" => "dot",
            "Tilde" => "~",
            "Ubrace" => "&#x23df;",
            _ => unreachable!(),
        };
        let body = unary_slot_text(v, f)?;
        return Ok(format!(
            "<i>{name}</i>{}",
            if present(v) {
                format!("<i>{body}</i>")
            } else {
                String::new()
            }
        ));
    }
    // Source UnaryFunction maps array children and joins LaTeX with spaces.
    // Validate the slot before calling the shared inherited implementation.
    let _ = unary_slot_text(v, f)?;
    unary_text(m, f)
}

fn attribute_pairs(m: &Model, all: bool) -> Result<Vec<(String, String)>, String> {
    let v = m.field("attributes");
    if !present(v) {
        return Ok(vec![]);
    }
    let map = v
        .as_object()
        .ok_or("source_render_error: accent attributes must support lookup")?;
    let mut pairs = vec![];
    for (k, value) in map {
        if !all && (k != "accent" || !present(value)) {
            continue;
        }
        let s = match value {
            Value::String(s) => s.clone(),
            Value::Bool(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            Value::Null => String::new(),
            _ => return Err("unported composite accent attribute value".into()),
        };
        pairs.push((k.clone(), s));
    }
    Ok(pairs)
}

pub(super) fn xml(m: &Model, f: &str, spacing: bool, templates: bool) -> Result<Vec<Xml>, String> {
    let v = m.field("parameter_one");
    let k = m.kind();
    if f == "mathml" {
        if k != "Ddot" {
            direct(v)?;
        }
        let glyph = match k {
            "Bar" => "&#xaf;",
            "Hat" => "^",
            "Vec" => "&#x2192;",
            "Dot" => ".",
            "Ddot" => "..",
            "Tilde" => "~",
            "Ubrace" => "&#x23df;",
            _ => unreachable!(),
        };
        let mark = el("mo", vec![xml_text(glyph)]);
        if !present(v) && ["Bar", "Hat", "Dot", "Ubrace"].contains(&k) {
            return Ok(vec![mark]);
        }
        let a = if k == "Ddot" {
            vec![("accent".into(), "true".into())]
        } else {
            attribute_pairs(m, ["Bar", "Hat", "Ubrace"].contains(&k))?
        };
        let mut body = if k == "Tilde" && !present(v) {
            vec![]
        } else {
            strict_slot_xml(v, f, spacing, templates, k == "Ddot")?
        };
        body.push(mark);
        return Ok(vec![attrs(
            if k == "Ubrace" { "munder" } else { "mover" },
            a,
            body,
        )]);
    }
    array_nodes(v)?;
    if !present(v) {
        return Ok(vec![run(match k {
            "Bar" => "&#xaf;",
            "Hat" => "^",
            "Vec" => "&#x2192;",
            "Dot" => ".",
            "Ddot" => "..",
            "Tilde" => "~",
            "Ubrace" => "⏟",
            _ => unreachable!(),
        })]);
    }
    let body = strict_slot_xml(v, f, spacing, templates, true)?;
    if k == "Hat" && present(m.field("hide_function_name")) {
        return Ok(body);
    }
    let attributes = m.field("attributes");
    if present(attributes) && !attributes.is_object() {
        return Err("source_render_error: accent attributes must support lookup".into());
    }
    let accent = present(&attributes["accent"]);
    if accent && !["Ddot", "Ubrace"].contains(&k) {
        let glyph = match k {
            "Bar" => "‾",
            "Hat" => "̂",
            "Vec" => "→",
            "Dot" => ".",
            "Tilde" => "˜",
            _ => unreachable!(),
        };
        return Ok(vec![el(
            "m:acc",
            vec![
                el(
                    "m:accPr",
                    vec![attrs("m:chr", vec![("m:val".into(), glyph.into())], vec![])],
                ),
                el("m:e", body),
            ],
        )]);
    }
    if k == "Bar" {
        return Ok(vec![el(
            "m:bar",
            vec![
                el(
                    "m:barPr",
                    vec![
                        attrs("m:pos", vec![("m:val".into(), "top".into())], vec![]),
                        control_properties(),
                    ],
                ),
                el("m:e", body),
            ],
        )]);
    }
    // Hat alone forwards display_style; the other source classes force true.
    if k == "Hat" && !templates {
        return Err("unported inline hat overset context".into());
    }
    let glyph = match k {
        "Hat" => "&#x302;",
        "Vec" => "→",
        "Dot" => ".",
        "Ddot" => "..",
        "Tilde" => "~",
        "Ubrace" => "⏟",
        _ => unreachable!(),
    };
    let tag = if k == "Ubrace" {
        "m:limLow"
    } else {
        "m:limUpp"
    };
    Ok(vec![el(
        tag,
        vec![
            el(&format!("{tag}Pr"), vec![control_properties()]),
            el("m:e", body),
            el("m:lim", vec![run(glyph)]),
        ],
    )])
}
