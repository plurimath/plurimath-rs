//! Ruby Uncertainty source procedures. Numeric lexemes bypass number formatting deliberately.
mod cloning;
use super::{attrs, el, run, text, Model, Value, WriterContext, Xml};
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn scalar(v: &Value) -> Result<String, String> {
    crate::source_values::to_s(&crate::source_values::decode(
        v,
        crate::source_values::Domain::Literal,
        crate::source_values::Profile::Canonical,
    )?)
}
/// Ruby text(number) calls number&.value, not Number's writer.
fn number(m: &Model, key: &str) -> Result<Option<String>, String> {
    let v = m.field(key);
    if v.is_null() {
        return Ok(None);
    }
    let n = Model::from_value(v).map_err(|_| {
        "source_render_error: undefined method 'value' for uncertainty number".to_string()
    })?;
    let value = n.field("value");
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(scalar(value)?))
    }
}
fn n(m: &Model, key: &str) -> Result<String, String> {
    Ok(number(m, key)?.unwrap_or_default())
}
fn category(m: &Model) -> &str {
    m.field("category").as_str().unwrap_or("")
}
fn concise(m: &Model) -> Result<String, String> {
    if !m.field("magnitude")["fields"]["value"].is_string() {
        return Err("source_render_error: uncertainty concise digits require String#delete".into());
    }
    let raw =
        number(m, "magnitude")?.ok_or("source_render_error: undefined method 'delete' for nil")?;
    let digits = raw.replace('.', "");
    let digits = digits.trim_start_matches('0');
    Ok(if digits.is_empty() {
        "0".into()
    } else {
        digits.into()
    })
}
fn parenthesised(m: &Model) -> Result<String, String> {
    if m.field("notation") == "concise" {
        concise(m)
    } else {
        n(m, "magnitude")
    }
}
fn marker(m: &Model) -> Result<String, String> {
    let mut name = match category(m) {
        "standard" => "standard-uncertainty",
        "expanded" => "expanded-uncertainty",
        "coverage_interval" => "coverage-interval",
        _ => "",
    }
    .to_string();
    if m.field("notation") == "concise" {
        name.push_str(":concise");
    }
    if truthy(m.field("interval_kind")) {
        name.push(':');
        name.push_str(match m.field("interval_kind").as_str() {
            Some("shortest") => "shortest",
            Some("probabilistically_symmetric") => "probabilistically-symmetric",
            _ => "",
        });
    }
    let keys: &[&str] = match category(m) {
        "standard" => &["value", "magnitude"],
        "expanded" => &[
            "value",
            "magnitude",
            "coverage_factor",
            "coverage_probability",
        ],
        _ => &["value", "lower", "upper", "coverage_probability"],
    };
    let mut args = Vec::new();
    for key in keys {
        if let Some(v) = number(m, key)? {
            args.push(v)
        }
    }
    if truthy(m.field("unit")) {
        args.push("$unit".into());
    }
    Ok(format!("{name}({})", args.join(",")))
}
fn statements(m: &Model) -> Vec<(&str, &str)> {
    [("k", "coverage_factor"), ("p", "coverage_probability")]
        .into_iter()
        .filter(|(_, k)| truthy(m.field(k)))
        .collect()
}
fn leading(m: &Model) -> Result<String, String> {
    if !m.field("value")["fields"]["value"].is_string() {
        return Err(
            "source_render_error: uncertainty leading value requires String#start_with?".into(),
        );
    }
    let v =
        number(m, "value")?.ok_or("source_render_error: undefined method 'start_with?' for nil")?;
    Ok(if v.starts_with('-') {
        format!("〖{v}〗")
    } else {
        v
    })
}
fn child(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    super::multiscripts::child(v, f, ctx)
}
fn unit_symbol(v: &Value) -> Option<String> {
    let m = Model::from_value(v).ok()?;
    if m.class == "Math::Function::FontStyle::Normal" {
        return unit_symbol(m.field("parameter_one"));
    }
    let raw = if m.class == "Math::Function::Text" {
        m.field("parameter_one")
    } else if m.class == "Math::Symbols::Symbol" {
        m.field("value")
    } else {
        return None;
    };
    let s = raw.as_str()?;
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphabetic()) {
        Some(s.into())
    } else {
        None
    }
}
fn unit_text(v: &Value) -> Option<String> {
    let m = Model::from_value(v).ok()?;
    if m.class == "Math::Formula" {
        let a = m.field("value").as_array()?;
        if a.is_empty() || a.len() % 2 == 0 {
            return None;
        }
        let mut out = String::new();
        for (i, v) in a.iter().enumerate() {
            if i % 2 == 1 {
                if v["class"] != "Math::Symbols::Cdot" {
                    return None;
                }
                out.push('*');
            } else {
                out.push_str(&unit_text(v)?);
            }
        }
        return Some(out);
    }
    if m.class == "Math::Function::Power" {
        let base = unit_symbol(m.field("parameter_one"))?;
        let e = m.field("parameter_two");
        let e = if e["class"] == "Math::Formula" {
            &e["fields"]["value"]
        } else {
            e
        };
        let mut a = if let Some(a) = e.as_array() {
            a.clone()
        } else if e.is_null() {
            vec![]
        } else {
            vec![e.clone()]
        };
        let sign = if a.len() == 2 && a[0]["class"] == "Math::Symbols::Minus" {
            a.remove(0);
            "-"
        } else {
            ""
        };
        if a.len() != 1 || a[0]["class"] != "Math::Number" {
            return None;
        }
        let s = a[0]["fields"]["value"].as_str()?.to_string();
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return Some(format!("{base}^{sign}{s}"));
    }
    unit_symbol(v)
}
fn asciimath_unit(v: &Value) -> Option<String> {
    if v["class"] == "Math::Function::Unitsml" {
        return v["fields"]["text"].as_str().map(str::to_string);
    }
    let candidate = unit_text(v)?;
    let formula = crate::units_parse::formula(&candidate, &serde_json::json!({})).ok()?;
    if unit_text(&formula).as_deref() == Some(&candidate) {
        Some(candidate)
    } else {
        None
    }
}
fn text_inner(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let cat = category(m);
    let v = n(m, "value")?;
    let mag = n(m, "magnitude")?;
    let u = m.field("unit");
    if f == "asciimath" {
        let mut args = match cat {
            "standard" => {
                if m.field("notation") == "concise" {
                    vec![Some(format!("{v}({})", concise(m)?))]
                } else {
                    vec![number(m, "value")?, number(m, "magnitude")?]
                }
            }
            "expanded" => vec![
                number(m, "value")?,
                number(m, "magnitude")?,
                number(m, "coverage_factor")?,
                number(m, "coverage_probability")?,
            ],
            _ => vec![
                number(m, "value")?,
                number(m, "lower")?,
                number(m, "upper")?,
                number(m, "coverage_probability")?,
                match m.field("interval_kind").as_str() {
                    Some("shortest") => Some("shortest".into()),
                    Some("probabilistically_symmetric") => Some("symmetric".into()),
                    _ => None,
                },
            ],
        }
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let unit = if truthy(u) { asciimath_unit(u) } else { None };
        if let Some(unit) = &unit {
            args.push(format!("\"unitsml({unit})\""));
        }
        let name = match cat {
            "standard" => "ustd",
            "expanded" => "uexp",
            "coverage_interval" => "ucov",
            _ => "",
        };
        let call = format!("{name}({})", args.join(", "));
        return if u.is_null() || unit.is_some() {
            Ok(call)
        } else {
            Ok(format!("{call} {}", child(u, f, ctx)?))
        };
    }
    let unit = if truthy(u) {
        if f == "unicodemath" {
            let model = Model::from_value(u)?;
            let mut value = if model.kind() == "Unitsml" {
                super::derived_units(&model)?.value()
            } else {
                model.value()
            };
            // unicodemath_unit is passed for both parsed and markup-derived units.
            super::unit_unicode_context(&mut value);
            let model = Model::from_value(&value)?;
            format!("ⓐ(unit {})", super::render_text(&model, f, ctx)?)
        } else {
            format!(" {}", child(u, f, ctx)?)
        }
    } else {
        String::new()
    };
    let mut output = match (cat, f) {
        ("standard", "unicodemath") => format!("{}({}){unit}", leading(m)?, parenthesised(m)?),
        ("standard", _) => format!("{v}({}){unit}", parenthesised(m)?),
        ("expanded", "latex") => format!("({v} \\pm {mag}){unit}"),
        ("expanded", "html") => format!("({v} &#xb1; {mag}){unit}"),
        ("expanded", "unicodemath") => format!("({v}±{mag}){unit}"),
        (_, "latex") => format!("{v},\\ [{}, {}]{unit}", n(m, "lower")?, n(m, "upper")?),
        (_, "unicodemath") => format!(
            "{}, [{}, {}]{unit}",
            leading(m)?,
            n(m, "lower")?,
            n(m, "upper")?
        ),
        (_, "html") => format!("{v}, [{}, {}]{unit}", n(m, "lower")?, n(m, "upper")?),
        _ => return Err("unsupported uncertainty text format".into()),
    };
    if cat != "standard" {
        for (name, key) in statements(m) {
            let num = n(m, key)?;
            output.push_str(&match f {
                "latex" => format!(",\\ {name} = {num}"),
                "unicodemath" => format!(", {name}={num}"),
                _ => format!(", {name} = {num}"),
            });
        }
    }
    if f == "unicodemath" {
        Ok(format!("ⓘ(\"{}\"{output})", marker(m)?))
    } else {
        Ok(output)
    }
}
fn fence(open: &str, close: &str, nodes: Vec<Xml>, f: &str) -> Xml {
    if f == "mathml" {
        let mut c = vec![el("mo", vec![text(open)])];
        c.extend(nodes);
        c.push(el("mo", vec![text(close)]));
        el("mrow", c)
    } else {
        el(
            "m:d",
            vec![
                el(
                    "m:dPr",
                    vec![
                        attrs("m:begChr", vec![("m:val".into(), open.into())], vec![]),
                        attrs("m:sepChr", vec![("m:val".into(), "".into())], vec![]),
                        attrs("m:endChr", vec![("m:val".into(), close.into())], vec![]),
                    ],
                ),
                el("m:e", nodes),
            ],
        )
    }
}
fn box_node(nodes: Vec<Xml>) -> Xml {
    el("m:box", vec![el("m:e", nodes)])
}
fn marker_node(m: &Model) -> Result<Xml, String> {
    Ok(el(
        "m:phant",
        vec![
            el(
                "m:phantPr",
                vec![
                    attrs("m:show", vec![("m:val".into(), "off".into())], vec![]),
                    attrs("m:zeroWid", vec![("m:val".into(), "on".into())], vec![]),
                    attrs("m:zeroAsc", vec![("m:val".into(), "on".into())], vec![]),
                    attrs("m:zeroDesc", vec![("m:val".into(), "on".into())], vec![]),
                ],
            ),
            el(
                "m:e",
                vec![el(
                    "m:r",
                    vec![
                        el(
                            "m:rPr",
                            vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
                        ),
                        el("m:t", vec![text(&marker(m)?)]),
                    ],
                )],
            ),
        ],
    ))
}
fn xml_inner(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let required: &[&str] = match category(m) {
        "standard" | "expanded" => &["value", "magnitude"],
        _ => &["value", "lower", "upper"],
    };
    for key in required
        .iter()
        .copied()
        .chain(statements(m).into_iter().map(|(_, key)| key))
    {
        if !m.field(key)["fields"]["value"].is_string() {
            return Err("source_render_error: XML number insertion requires a String".into());
        }
    }
    let number = |s: String| {
        if f == "mathml" {
            el("mn", vec![text(&s)])
        } else {
            run(&s)
        }
    };
    let op = |s: &str| {
        if f == "mathml" {
            el("mo", vec![text(s)])
        } else {
            run(s)
        }
    };
    let mut nodes = if f == "omml" {
        vec![marker_node(m)?]
    } else {
        vec![]
    };
    nodes.extend(match category(m) {
        "standard" => vec![
            number(n(m, "value")?),
            fence("(", ")", vec![number(parenthesised(m)?)], f),
        ],
        "expanded" => vec![fence(
            "(",
            ")",
            vec![
                number(n(m, "value")?),
                op("&#xb1;"),
                number(n(m, "magnitude")?),
            ],
            f,
        )],
        _ => vec![
            number(n(m, "value")?),
            op(","),
            fence(
                "[",
                "]",
                vec![number(n(m, "lower")?), op(","), number(n(m, "upper")?)],
                f,
            ),
        ],
    });
    if truthy(m.field("unit")) {
        let mut child = super::render_xml(&Model::from_value(m.field("unit"))?, f, ctx)?;
        if f == "mathml" {
            let mut row =
                if child.len() == 1 && matches!(&child[0],Xml::El(name,_,_) if name=="mrow") {
                    child.remove(0)
                } else {
                    el("mrow", child)
                };
            if let Xml::El(_, a, _) = &mut row {
                // Formula's source XML postprocessor consumes the unitsml marker.
                a.retain(|(key, _)| key != "unitsml");
                a.push(("arg".into(), "unit".into()));
            }
            nodes.push(attrs(
                "mo",
                vec![("rspace".into(), "thickmathspace".into())],
                vec![text("&#x2062;")],
            ));
            nodes.push(row);
        } else {
            nodes.push(box_node(child));
        }
    }
    for (name, key) in statements(m) {
        nodes.push(op(","));
        nodes.push(if f == "mathml" {
            el("mi", vec![text(name)])
        } else {
            run(name)
        });
        nodes.push(op("="));
        nodes.push(number(n(m, key)?));
    }
    if f == "mathml" {
        Ok(vec![attrs(
            "mrow",
            vec![("intent".into(), marker(m)?)],
            nodes,
        )])
    } else {
        Ok(vec![box_node(nodes)])
    }
}

// Existing explicit per-call adapter reproduces Formula's public error wrapper.
fn source_error(error: String, ctx: &WriterContext) -> String {
    if let Some(cause) = error.strip_prefix("source_render_error: ") {
        super::formatter_context::source_error(ctx, "NoMethodError", cause.into())
    } else {
        error
    }
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    text_inner(m, f, ctx).map_err(|e| source_error(e, ctx))
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    xml_inner(m, f, ctx).map_err(|e| source_error(e, ctx))
}
pub(super) fn before_split(m: &Model, f: &str, ctx: &WriterContext) -> Result<(), String> {
    cloning::before_split(&m.value(), f, ctx)
}
