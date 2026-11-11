//! Source layout/container/suppression procedures (Ruby 0a1df68a).
use super::{attrs, el, run, Model, Value, WriterContext, Xml};
use crate::source_values::{self as source, Domain, Profile, RubyValue};
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn child(v: &Value, f: &str) -> Result<String, String> {
    super::multiscripts::child(v, f)
}
fn xml_child(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    super::multiscripts::xml_child(v, f, ctx)
}
fn data() -> &'static Value {
    static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/layout-nodes/source-declarations.json"
        ))
        .expect("source layout declarations")
    })
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Msgroup"
            | "Math::Function::Merror"
            | "Math::Function::Longdiv"
            | "Math::Function::Mpadded"
            | "Math::Function::Scarries"
            | "Math::Function::Rule"
    )
}
fn array_text(v: &Value, f: &str, compact: bool, separator: &str) -> Result<String, String> {
    let values = v
        .as_array()
        .ok_or("source_render_error: layout field has no map method")?;
    values
        .iter()
        .filter(|v| !compact || !v.is_null())
        .map(|v| child(v, f))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| v.join(separator))
}
fn unary_text(v: &Value, f: &str) -> Result<String, String> {
    if f == "asciimath" && !truthy(v) {
        return Ok(String::new());
    }
    if v.is_array() {
        return array_text(v, f, true, if f == "latex" { " " } else { "" });
    }
    if v.is_null() {
        Ok(String::new())
    } else {
        child(v, f)
    }
}
fn unary_html(m: &Model) -> Result<String, String> {
    let v = m.field("parameter_one");
    let name = m.kind().to_ascii_lowercase();
    let mut result = format!("<i>{name}</i>");
    if v.is_array() {
        result.push_str(&format!("<i>{}</i>", array_text(v, "html", false, "")?));
    } else if truthy(v) {
        result.push_str(&format!("<i>{}</i>", child(v, "html")?));
    }
    Ok(result)
}
// Canonical maps use the source-declared keyword/attribute key domains;
// literal JSON values retain their actual String kind. Lossless Ruby object
// variants use the separate explicit profile rather than shape inference.
fn bind_padded(value: RubyValue, profile: Profile) -> Result<RubyValue, String> {
    if let RubyValue::Hash(mut entries) = value {
        for (key, value) in &mut entries {
            if matches!(key,RubyValue::Symbol(name)if name=="mpadded")
                && matches!(value, RubyValue::Hash(_))
            {
                *value = source::in_domain(value, Domain::NamedOptions, profile)?;
            }
        }
        Ok(RubyValue::Hash(entries))
    } else {
        Ok(value)
    }
}
fn source_options(m: &Model) -> Result<RubyValue, String> {
    bind_padded(
        source::decode(m.field("options"), Domain::NamedOptions, Profile::Canonical)?,
        Profile::Canonical,
    )
}
fn padded_unicode(m: &Model) -> Result<String, String> {
    let v = m.field("parameter_one");
    let options = source_options(m)?;
    if !matches!(options, RubyValue::Nil | RubyValue::Hash(_)) {
        return Err("source_render_error: Mpadded options have no Hash dig method".into());
    }
    if options
        .get_symbol("mpadded")
        .is_some_and(|v| !matches!(v, RubyValue::Nil | RubyValue::Bool(false)))
    {
        let mut prefix = "";
        for (name, value) in data()["phantom_options"].as_object().unwrap() {
            let declaration = bind_padded(
                source::hash(value, Domain::NamedOptions, Profile::Canonical)?,
                Profile::Canonical,
            )?;
            if source::equal(&declaration, &options) {
                prefix = data()["unary_symbols"][name].as_str().unwrap_or("");
                break;
            }
        }
        let body = child(v, "unicodemath")?;
        let body = if v["class"]
            .as_str()
            .is_some_and(|c| c.starts_with("Math::Function::Fenced"))
        {
            body
        } else {
            format!("({body})")
        };
        return Ok(format!("{prefix}{body}"));
    }
    if let Some(mask) = options.get_symbol("mask") {
        let body = if v.is_null() {
            String::new()
        } else {
            child(v, "unicodemath")?
        };
        return Ok(format!("⟡({}&{body})", source::to_s(mask)?));
    }
    Ok(if truthy(v) {
        format!("⟡({})", child(v, "unicodemath")?)
    } else {
        "⟡".into()
    })
}
pub(super) fn text_writer(m: &Model, f: &str) -> Result<String, String> {
    let v = m.field("parameter_one");
    if f == "html" {
        if matches!(m.kind(), "Rule" | "Msgroup") {
            return Err(
                "source_argument_error: layout to_html does not accept options keyword".into(),
            );
        }
        return unary_html(m);
    }
    match m.kind() {
        "Merror" => Ok(String::new()),
        "Rule" => {
            if f != "latex" {
                return Ok(String::new());
            }
            let mut result = "\\rule".to_string();
            for (field, open, close) in [
                ("parameter_one", "[", "]"),
                ("parameter_two", "{", "}"),
                ("parameter_three", "{", "}"),
            ] {
                if truthy(m.field(field)) {
                    result.push_str(&format!("{open}{}{close}", child(m.field(field), f)?));
                }
            }
            Ok(result)
        }
        "Msgroup" => array_text(v, f, false, ""),
        "Longdiv" | "Scarries" | "Mpadded" => match f {
            "asciimath" | "latex" => unary_text(v, f),
            "unicodemath" if m.kind() == "Mpadded" => padded_unicode(m),
            "unicodemath" => {
                let value = if v.is_null() {
                    String::new()
                } else {
                    child(v, f)?
                };
                Ok(if m.kind() == "Scarries" {
                    format!("scarries⁡{value}")
                } else {
                    value
                })
            }
            _ => Err(format!("unported layout text format {f}")),
        },
        _ => Err(format!("unported layout class {}", m.class)),
    }
}
fn unary_xml(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let mut out = vec![];
    if let Some(values) = v.as_array() {
        for v in values.iter().filter(|v| !v.is_null()) {
            out.extend(xml_child(v, f, ctx)?);
        }
    } else if !v.is_null() {
        out.extend(xml_child(v, f, ctx)?);
    }
    Ok(out)
}
fn attribute_pairs(m: &Model) -> Result<Vec<(String, String)>, String> {
    let value = source_options(m)?;
    match value {
        RubyValue::Nil => Ok(vec![]),
        RubyValue::Hash(pairs) => pairs
            .iter()
            .map(|(k, v)| Ok((source::to_s(k)?, source::to_s(v)?)))
            .collect(),
        RubyValue::Array(items) => {
            let mut result = vec![];
            for item in items {
                let RubyValue::Array(pair) = item else {
                    return Err(
                        "source_argument_error: layout attribute Array entry must be a pair".into(),
                    );
                };
                if pair.len() != 2 {
                    return Err(
                        "source_argument_error: layout attribute Array entry must be a pair".into(),
                    );
                }
                result.push((source::to_s(&pair[0])?, source::to_s(&pair[1])?));
            }
            Ok(result)
        }
        _ => Err("source_render_error: layout attribute container has no each method".into()),
    }
}
fn padded_omml(m: &Model, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let mut properties = vec![];
    let value = source_options(m)?;
    let pairs = match value {
        RubyValue::Nil => vec![],
        RubyValue::Hash(pairs) => pairs,
        RubyValue::Array(items) => {
            let mut result = vec![];
            for item in items {
                let RubyValue::Array(pair) = item else {
                    return Err(
                        "source_render_error: Mpadded option value has no match? method".into(),
                    );
                };
                let key = pair.first().cloned().unwrap_or(RubyValue::Nil);
                let value = pair.get(1).cloned().unwrap_or(RubyValue::Nil);
                result.push((key, value));
            }
            result
        }
        _ => return Err("source_render_error: Mpadded options have no map method".into()),
    };
    for (key, value) in pairs {
        let raw = match &value {
            RubyValue::String(s) => s,
            _ => return Err("source_render_error: Mpadded option has no match? method".into()),
        };
        if !raw.bytes().any(|c| (b'1'..=b'9').contains(&c))
            && raw.bytes().any(|c| c.is_ascii_digit())
        {
            // Preserve source-profile returned bytes, including an empty
            // name for nil ZERO_TAGS lookup. This is not schema validation.
            let tag = match key {
                RubyValue::Symbol(key) => data()["zero_tags"][key.as_str()].as_str().unwrap_or(""),
                _ => "",
            };
            properties.push(attrs(tag, vec![("m:val".into(), "on".into())], vec![]));
        }
    }
    let mut children = vec![];
    if !properties.is_empty() {
        children.push(el("m:phantPr", properties));
    }
    let v = m.field("parameter_one");
    let body = if !truthy(v) {
        vec![run("&#8203;")]
    } else if let Some(a) = v.as_array() {
        let mut out = vec![];
        for v in a {
            out.extend(xml_child(v, "omml", ctx)?);
        }
        out
    } else {
        xml_child(v, "omml", ctx)?
    };
    children.push(el("m:e", body));
    Ok(vec![el("m:phant", children)])
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let v = m.field("parameter_one");
    if m.kind() == "Rule" {
        return Ok(vec![if f == "mathml" {
            el("mi", vec![])
        } else {
            el("m:r", vec![el("m:t", vec![])])
        }]);
    }
    if f == "omml" {
        return match m.kind() {
            "Merror" => Ok(vec![]),
            "Mpadded" => padded_omml(m, ctx),
            _ => unary_xml(v, f, ctx),
        };
    }
    if f != "mathml" {
        return Err(format!("unported layout XML format {f}"));
    }
    let children = if m.kind() == "Msgroup" {
        let values = v
            .as_array()
            .ok_or("source_render_error: Msgroup field has no map method")?;
        let mut out = vec![];
        for value in values.iter().filter(|v| !v.is_null()) {
            out.extend(xml_child(value, f, ctx)?);
        }
        out
    } else {
        unary_xml(v, f, ctx)?
    };
    let tag = match m.kind() {
        "Msgroup" => "msgroup",
        "Merror" => "merror",
        "Longdiv" => "mlongdiv",
        "Scarries" => "mscarries",
        "Mpadded" => "mpadded",
        _ => return Err("unported layout XML class".into()),
    };
    let properties = if m.kind() == "Mpadded" {
        attribute_pairs(m)?
    } else {
        vec![]
    };
    Ok(vec![attrs(tag, properties, children)])
}
