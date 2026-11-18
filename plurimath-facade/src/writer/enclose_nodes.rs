//! Source Menclose, Overleftrightarrow and Semantics procedures.
use super::{
    attrs, child, el, omml_slot, overset_xml, parens, render_xml, run, strict_slot_xml, text,
    Model, Value, WriterContext, Xml,
};
use serde_json::json;

pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Menclose"
            | "Math::Function::Overleftrightarrow"
            | "Math::Function::Semantics"
    )
}

pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    match m.kind() {
        "Menclose" => menclose_text(m, f, ctx),
        "Overleftrightarrow" => overleftrightarrow_text(m, f, ctx),
        _ => semantics_text(m, f, ctx),
    }
}

pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    match m.kind() {
        "Menclose" => menclose_xml(m, f, ctx),
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

/// Menclose#parameter_one is the notation string; nil interpolates as "".
fn notation(m: &Model) -> Result<Option<&str>, String> {
    match m.field("parameter_one") {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        _ => Err("unported promised Menclose notation value".into()),
    }
}

// Utility::UNICODEMATH_MENCLOSE_FUNCTIONS in key order; Hash#key returns the
// first key, so "bottom" is underline and "circle" is ellipse. The symbol is
// UNARY_ARG_FUNCTIONS[key] || UNARY_SYMBOLS[key].
const UNICODE_MENCLOSE: [(&str, &str); 10] = [
    ("bottom", "&#x2581;"),
    ("bottom", "&#x2581;"),
    ("longdiv", "&#x27cc;"),
    ("updiagonalstrike downdiagonalstrike", "&#x2573;"),
    ("updiagonalstrike", "&#x2572;"),
    ("circle", "&#x2b2d;"),
    ("circle", "&#x2b2d;"),
    ("downdiagonalstrike", "&#x2571;"),
    ("roundedbox", "&#x25a2;"),
    ("box", "&#x25ad;"),
];
// Utility::MASK_CLASSES.
const MASK_CLASSES: [(u32, &str); 8] = [
    (1, "top"),
    (2, "bottom"),
    (4, "left"),
    (8, "right"),
    (16, "horizontalstrike"),
    (32, "verticalstrike"),
    (64, "downdiagonalstrike"),
    (128, "updiagonalstrike"),
];

fn menclose_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let value = node_slot(m, "parameter_two")?;
    match f {
        "asciimath" | "latex" => child(value, f, ctx),
        "html" => Ok(format!(
            "<menclose notation=\"{}\">{}</menclose>",
            notation(m)?.unwrap_or(""),
            child(value, f, ctx)?
        )),
        "unicodemath" => {
            let Some(notation) = notation(m)? else {
                return Err(
                    "source_no_method_error: Menclose notation nil has no include? method".into(),
                );
            };
            if let Some((_, symbol)) = UNICODE_MENCLOSE.iter().find(|(n, _)| *n == notation) {
                return Ok(format!("{symbol}{}", parens(value, f, ctx)?));
            }
            if MASK_CLASSES.iter().any(|(_, mask)| notation.contains(mask)) {
                // ModelHelper.notations_to_mask: known words summed, XOR 15.
                let mask = notation
                    .split_whitespace()
                    .filter_map(|word| {
                        MASK_CLASSES
                            .iter()
                            .find(|(_, name)| *name == word)
                            .map(|(bit, _)| *bit)
                    })
                    .sum::<u32>()
                    ^ 15;
                return Ok(format!("▭({mask}&{})", child(value, f, ctx)?));
            }
            // Neither branch matches: the source method returns nil.
            Ok(String::new())
        }
        _ => Err(format!("unknown Menclose text format {f}")),
    }
}

fn on(name: &str) -> Xml {
    attrs(
        &format!("m:{name}"),
        vec![("m:val".into(), "on".into())],
        vec![],
    )
}

fn menclose_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = node_slot(m, "parameter_two")?;
    let notation = notation(m)?;
    if f == "mathml" {
        return Ok(vec![attrs(
            "menclose",
            vec![("notation".into(), notation.unwrap_or("").to_owned())],
            strict_slot_xml(value, f, ctx, false)?,
        )]);
    }
    let Some(notation) = notation else {
        return Err("source_no_method_error: Menclose notation nil has no include? method".into());
    };
    let mut nodes = vec![];
    // borderboxpr: exact box/circle/roundedbox have no properties element.
    if !["box", "circle", "roundedbox"].contains(&notation) {
        let mut props = vec![];
        if !["box", "circle", "roundedbox"]
            .iter()
            .any(|v| notation.contains(v))
        {
            for (side, name) in [
                ("top", "hideTop"),
                ("bottom", "hideBot"),
                ("left", "hideLeft"),
                ("right", "hideRight"),
            ] {
                if !notation.contains(side) {
                    props.push(on(name));
                }
            }
        }
        for (strike, name) in [
            ("horizontalstrike", "strikeH"),
            ("verticalstrike", "strikeV"),
            ("updiagonalstrike", "strikeBLTR"),
            ("downdiagonalstrike", "strikeTLBR"),
        ] {
            if notation.contains(strike) {
                props.push(on(name));
            }
        }
        nodes.push(el("m:borderBoxPr", props));
    }
    nodes.push(el("m:e", omml_slot(value, f, ctx)?));
    Ok(vec![el("m:borderBox", nodes)])
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
