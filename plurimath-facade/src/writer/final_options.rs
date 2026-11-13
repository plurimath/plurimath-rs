//! Source procedures for Limits, Underset, Underover and Frac option profiles.
use super::{attrs, control_properties, el, Model, Value, WriterContext, Xml};
use crate::source_values::{self as source, Domain, Profile, RubyValue};
pub(super) fn data() -> &'static Value {
    static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/final-writer-options/source-declarations.json"
        ))
        .expect("source writer declarations")
    })
}
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn child(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    super::multiscripts::child(v, f, ctx)
}
fn safe(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if v.is_null() {
        Ok(String::new())
    } else {
        child(v, f, ctx)
    }
}
pub(super) fn parens(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    let value = child(v, "unicodemath", ctx)?;
    Ok(if v["class"] == "Math::Function::Fenced" {
        value
    } else {
        format!("({value})")
    })
}
fn first_wrap(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let value = child(v, f, ctx)?;
    if v["class"] == "Math::Formula" {
        Ok(if f == "latex" {
            format!("{{{value}}}")
        } else {
            format!("({value})")
        })
    } else {
        Ok(value)
    }
}
fn named_options(m: &Model) -> Result<RubyValue, String> {
    source::field(m, "options", Domain::NamedOptions)
}
fn ruby_truthy(v: &RubyValue) -> bool {
    !matches!(v, RubyValue::Nil | RubyValue::Bool(false))
}
fn lookup<'a>(v: &'a RubyValue, key: &str) -> Result<Option<&'a RubyValue>, String> {
    match v {
        RubyValue::Hash(_) => Ok(v.get_symbol(key)),
        _ => Err("source_render_error: option receiver cannot index a Symbol key".into()),
    }
}
fn attribute_pairs(
    value: &RubyValue,
    remove_choose: bool,
) -> Result<Vec<(String, String)>, String> {
    let pairs = match value {
        RubyValue::Hash(pairs) => pairs.clone(),
        RubyValue::Array(values) => values
            .iter()
            .map(|value| match value {
                RubyValue::Array(pair) if pair.len() == 2 => Ok((pair[0].clone(), pair[1].clone())),
                _ => Err("source_render_error: XML attributes require key/value pairs".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?,
        RubyValue::Nil => return Ok(vec![]),
        _ => return Err("source_render_error: XML attributes do not support this receiver".into()),
    };
    pairs
        .iter()
        .filter(|(key, _)| !remove_choose || !matches!(key,RubyValue::Symbol(key)if key=="choose"))
        .map(|(key, value)| Ok((source::to_s(key)?, source::to_s(value)?)))
        .collect()
}
pub(super) fn raw_symbol(v: &Value) -> Result<Option<String>, String> {
    let Ok(m) = Model::from_value(v) else {
        return Ok(None);
    };
    if !m.class.starts_with("Math::Symbols::") {
        return Ok(None);
    }
    if m.class == "Math::Symbols::Symbol" {
        return Ok(m.field("value").as_str().map(str::to_owned));
    }
    Ok(data()["symbol_hex"][&m.class].as_str().map(str::to_owned))
}
fn under_unicode(m: &Model, ctx: &super::WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let raw = raw_symbol(a)?;
    if raw.as_ref().is_some_and(|s| {
        data()["horizontal_brackets"]
            .as_object()
            .unwrap()
            .values()
            .any(|v| v == s)
    }) {
        return Ok(format!(
            "{}{}",
            child(a, "unicodemath", ctx)?,
            parens(b, ctx)?
        ));
    }
    if ["Math::Function::Obrace", "Math::Function::Ubrace"]
        .iter()
        .any(|kind| b["class"] == *kind)
    {
        return Ok(format!(
            "{}_{}",
            child(b, "unicodemath", ctx)?,
            parens(a, ctx)?
        ));
    }
    if let Some(raw) = raw {
        if data()["diacritic_belows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == &raw)
            || data()["accent_symbols"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v == &raw)
        {
            return Ok(format!("{}{raw}", parens(b, ctx)?));
        }
    }
    Ok(format!(
        "{}┬{}",
        parens(b, ctx)?,
        child(a, "unicodemath", ctx)?
    ))
}
fn source_integer(v: &RubyValue) -> Result<i128, String> {
    let text = match v {
        RubyValue::Nil => return Ok(0),
        RubyValue::Integer(value) | RubyValue::String(value) => value,
        RubyValue::Float(bits) => {
            let value = f64::from_bits(*bits);
            if !value.is_finite() {
                return Err("source_render_error: FloatDomainError in fraction to_i".into());
            }
            return Ok(value.trunc() as i128);
        }
        _ => return Err("source_render_error: fraction value has no to_i method".into()),
    };
    let bytes = text.as_bytes();
    let mut index = 0;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    let negative = bytes.get(index) == Some(&b'-');
    if matches!(bytes.get(index), Some(b'+') | Some(b'-')) {
        index += 1;
    }
    if bytes.get(index) == Some(&b'0') && matches!(bytes.get(index + 1), Some(b'd') | Some(b'D')) {
        index += 2;
    }
    let mut number = 0i128;
    let mut seen = false;
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_digit() {
            number = number
                .saturating_mul(10)
                .saturating_add((byte - b'0') as i128);
            seen = true;
            index += 1;
        } else if *byte == b'_' && seen && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            index += 1;
        } else {
            break;
        }
    }
    Ok(if negative { -number } else { number })
}
fn fraction_lookup(m: &Model) -> Result<String, String> {
    let mut fields = vec![];
    for key in ["parameter_one", "parameter_two"] {
        let node = Model::from_value(m.field(key))?;
        let value_key =
            if ["Math::Function::Text", "Math::Function::Ms"].contains(&node.class.as_str()) {
                "parameter_one"
            } else if ["Math::Number", "Math::Formula", "Math::Symbols::Symbol"]
                .contains(&node.class.as_str())
                || data()["symbol_hex"].get(&node.class).is_some()
            {
                "value"
            } else {
                return Err(format!(
                    "unported promised fraction value accessor for {}",
                    node.class
                ));
            };
        fields.push(source_integer(&source::field(
            &node,
            value_key,
            Domain::Literal,
        )?)?);
    }
    Ok(data()["unicode_fractions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, v)| {
            v.as_array().is_some_and(|v| {
                v.len() == 2
                    && v[0].as_i64().map(i128::from) == Some(fields[0])
                    && v[1].as_i64().map(i128::from) == Some(fields[1])
            })
        })
        .map(|(key, _)| key.clone())
        .unwrap_or_default())
}
fn fraction_unicode(m: &Model, ctx: &super::WriterContext) -> Result<String, String> {
    let options = named_options(m)?;
    if !matches!(options, RubyValue::Nil)
        && lookup(&options, "unicodemath_fraction")?.is_some_and(ruby_truthy)
    {
        return fraction_lookup(m);
    }
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let first = if truthy(a) {
        parens(a, ctx)?
    } else {
        String::new()
    };
    let second = if truthy(b) {
        parens(b, ctx)?
    } else {
        String::new()
    };
    if !truthy(m.field("options")) {
        return Ok(format!("{first}/{second}"));
    }
    if lookup(&options, "linethickness")?.is_some() {
        return Ok(format!("{first}¦{second}"));
    }
    if lookup(&options, "displaystyle")?.is_some() {
        return Ok(format!(
            "{}⊘{}",
            child(a, "unicodemath", ctx)?,
            child(b, "unicodemath", ctx)?
        ));
    }
    if lookup(&options, "ldiv")?.is_some() {
        return Ok(format!("{first}∕{second}"));
    }
    Ok(String::new())
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Limits"
            | "Math::Function::Underset"
            | "Math::Function::Underover"
            | "Math::Function::Frac"
    )
}
pub(super) fn text_writer(
    m: &Model,
    f: &str,
    ctx: &super::WriterContext,
) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    if f == "html" {
        let mut out = String::new();
        let fields = if matches!(m.kind(), "Limits" | "Underover") {
            vec![a, b, c]
        } else {
            vec![a, b]
        };
        for v in fields {
            if truthy(v) {
                out.push_str(&format!("<i>{}</i>", child(v, f, ctx)?));
            }
        }
        return Ok(out);
    }
    if m.kind() == "Frac" {
        return match f {
            "asciimath" => {
                let mut out = "frac".to_string();
                for v in [a, b] {
                    if truthy(v) {
                        out.push_str(&format!("({})", child(v, f, ctx)?));
                    }
                }
                Ok(out)
            }
            "latex" => Ok(format!(
                "\\frac{{{}}}{{{}}}",
                safe(a, f, ctx)?,
                safe(b, f, ctx)?
            )),
            "unicodemath" => fraction_unicode(m, ctx),
            _ => Err("unported fraction text format".into()),
        };
    }
    if m.kind() == "Underset" {
        return match f {
            "asciimath" => {
                let mut out = "underset".to_string();
                for v in [a, b] {
                    if truthy(v) {
                        out.push_str(&format!("({})", child(v, f, ctx)?));
                    }
                }
                Ok(out)
            }
            "latex" => {
                let mut out = "\\underset".to_string();
                for v in [a, b] {
                    if truthy(v) {
                        out.push_str(&super::nary::latex_wrapped_with_value(
                            v,
                            child(v, f, ctx)?,
                        )?);
                    }
                }
                Ok(out)
            }
            "unicodemath" => under_unicode(m, ctx),
            _ => Err("unported Underset text format".into()),
        };
    }
    match f {
        "asciimath" | "latex" if !(f == "latex" && m.kind() == "Limits") => {
            let mut out = if truthy(a) {
                first_wrap(a, f, ctx)?
            } else {
                String::new()
            };
            for (v, mark) in [(b, "_"), (c, "^")] {
                if truthy(v) {
                    let body = child(v, f, ctx)?;
                    out.push_str(&format!(
                        "{mark}{}",
                        if f == "latex" {
                            format!("{{{body}}}")
                        } else {
                            format!("({body})")
                        }
                    ));
                }
            }
            Ok(out)
        }
        "latex" => Ok(format!(
            "{}\\limits_{}^{}",
            safe(a, f, ctx)?,
            if truthy(b) {
                format!("{{{}}}", child(b, f, ctx)?)
            } else {
                String::new()
            },
            if truthy(c) {
                format!("{{{}}}", child(c, f, ctx)?)
            } else {
                String::new()
            }
        )),
        "unicodemath" => {
            let mut out = safe(a, f, ctx)?;
            if m.kind() == "Limits" {
                if truthy(c) {
                    let no_wrap = c["class"] == "Math::Function::Power"
                        || (a["class"] == "Math::Function::Power"
                            && super::prime_symbol(&a["fields"]["parameter_two"], ctx)?);
                    out.push_str(&format!(
                        "┴{}",
                        if no_wrap {
                            child(c, f, ctx)?
                        } else {
                            parens(c, ctx)?
                        }
                    ));
                }
                if truthy(b) {
                    out.push_str(&format!(
                        "┬{}",
                        if b["class"] == "Math::Function::Base" {
                            child(b, f, ctx)?
                        } else {
                            parens(b, ctx)?
                        }
                    ));
                }
            } else {
                for (v, mark) in [(b, "┬"), (c, "┴")] {
                    if truthy(v) {
                        out.push_str(&format!("{mark}{}", parens(v, ctx)?));
                    }
                }
            }
            Ok(out)
        }
        _ => Err("unported ternary writer format".into()),
    }
}
fn direct(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if v.is_array() {
        return Err("source_render_error: Array cannot receive direct XML writer".into());
    }
    super::generic_nary::slot(v, f, ctx)
}
fn validate(v: &Value, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    super::generic_nary::slot(v, "mathml", ctx)
}
fn parameter(v: &Value, name: &str, ctx: &WriterContext) -> Result<Xml, String> {
    Ok(el(name, super::generic_nary::slot(v, "omml", ctx)?))
}
fn fraction_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    if f == "mathml" {
        let mut body = direct(a, f, ctx)?;
        body.extend(direct(b, f, ctx)?);
        let hidden = truthy(m.field("hide_function_name"));
        let attributes = if !hidden && truthy(m.field("options")) {
            attribute_pairs(&named_options(m)?, true)?
        } else {
            vec![]
        };
        return Ok(vec![attrs(
            if hidden { "mrow" } else { "mfrac" },
            attributes,
            body,
        )]);
    }
    let mut properties = vec![];
    if truthy(m.field("options")) {
        let options = named_options(m)?;
        let kind = if lookup(&options, "linethickness")? == Some(&RubyValue::String("0".into())) {
            "noBar"
        } else if lookup(&options, "bevelled")? == Some(&RubyValue::String("true".into())) {
            "skw"
        } else {
            "bar"
        };
        properties.push(attrs("m:type", vec![("m:val".into(), kind.into())], vec![]));
    }
    properties.push(control_properties());
    Ok(vec![el(
        "m:f",
        vec![
            el("m:fPr", properties),
            parameter(a, "m:num", ctx)?,
            parameter(b, "m:den", ctx)?,
        ],
    )])
}
fn underset_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    if f == "mathml" {
        let mut body = validate(b, ctx)?;
        body.extend(validate(a, ctx)?);
        return Ok(vec![attrs(
            "munder",
            attribute_pairs(&named_options(m)?, false)?,
            body,
        )]);
    }
    if !ctx.display_style {
        return super::render_xml(
            &Model::from_value(&crate::model::node(
                "Math::Function::Base",
                serde_json::json!({"parameter_one":b,"parameter_two":a}),
            ))?,
            f,
            ctx,
        );
    }
    let options = named_options(m)?;
    if lookup(&options, "accentunder")?.is_some_and(ruby_truthy) {
        return Ok(vec![el(
            "m:groupChr",
            vec![
                el(
                    "m:groupChrPR",
                    vec![
                        attrs("m:chr", vec![("m:val".into(), "_".into())], vec![]),
                        attrs("m:pos", vec![("m:val".into(), "bot".into())], vec![]),
                    ],
                ),
                parameter(b, "m:e", ctx)?,
            ],
        )]);
    }
    Ok(vec![el(
        "m:limLow",
        vec![
            el("m:limLowPr", vec![control_properties()]),
            parameter(b, "m:e", ctx)?,
            parameter(a, "m:lim", ctx)?,
        ],
    )])
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if m.kind() == "Frac" {
        return fraction_xml(m, f, ctx);
    }
    if m.kind() == "Underset" {
        return underset_xml(m, f, ctx);
    }
    if f == "mathml" {
        let mut body = vec![];
        for key in ["parameter_one", "parameter_two", "parameter_three"] {
            body.extend(validate(m.field(key), ctx)?);
        }
        return Ok(vec![el("munderover", body)]);
    }
    // TernaryFunction#underover delegates upper/base to Overset, then lower to Underset.
    if m.kind() == "Underover" && !ctx.display_style {
        return super::render_xml(
            &Model::from_value(&crate::model::node(
                "Math::Function::PowerBase",
                serde_json::json!({"parameter_one":m.field("parameter_one"),"parameter_two":m.field("parameter_two"),"parameter_three":m.field("parameter_three")}),
            ))?,
            f,
            ctx,
        );
    }
    let over = crate::model::node(
        "Math::Function::Overset",
        serde_json::json!({"parameter_one":m.field("parameter_three"),"parameter_two":m.field("parameter_one"),"options":{}}),
    );
    if !truthy(m.field("parameter_two")) {
        return super::render_xml(&Model::from_value(&over)?, f, ctx);
    }
    underset_xml(
        &Model::from_value(&crate::model::node(
            "Math::Function::Underset",
            serde_json::json!({"parameter_one":m.field("parameter_two"),"parameter_two":over,"options":{}}),
        ))?,
        f,
        ctx,
    )
}

/// Core#prime_unicode? compares declared source spellings, never decoded glyphs.
pub(super) fn source_prime(value: &Value) -> Result<bool, String> {
    let Ok(model) = Model::from_value(value) else {
        return Ok(false);
    };
    if !model.class.starts_with("Math::Symbols::") {
        return Ok(false);
    }
    fn contains(value: &RubyValue, needle: &str) -> Result<bool, String> {
        let needle = RubyValue::String(needle.into());
        match value {
            RubyValue::String(value) => {
                if let RubyValue::String(needle) = needle {
                    Ok(value.contains(&needle))
                } else {
                    unreachable!()
                }
            }
            RubyValue::Array(values) => {
                Ok(values.iter().any(|value| source::equal(value, &needle)))
            }
            RubyValue::Hash(values) => Ok(values.iter().any(|(key, _)| source::eql(key, &needle))),
            _ => Err("source_render_error: prime source value has no include? method".into()),
        }
    }
    let declared = source::field(&model, "value", Domain::Literal)?;
    if !matches!(declared, RubyValue::Nil) && contains(&declared, "&#x27;")? {
        return Ok(true);
    }
    let raw = if model.class == "Math::Symbols::Symbol" {
        declared
    } else {
        source::decode(
            &data()["symbol_hex"][&model.class],
            Domain::Literal,
            Profile::Canonical,
        )?
    };
    for prime in data()["primes"]
        .as_object()
        .ok_or("source prime declarations absent")?
        .values()
    {
        if contains(
            &raw,
            prime
                .as_str()
                .ok_or("source prime spelling is not a String")?,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn underset_line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    super::source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
    if !obj.is_empty() {
        *obj = vec![super::new_source_node(
            "Math::Function::Underset",
            serde_json::json!({"parameter_one":super::filter_source_values(std::mem::take(obj)),"parameter_two":node["fields"]["parameter_two"],"options":{}}),
        )];
        node["fields"]["parameter_two"] = Value::Null;
        return Ok(());
    }
    super::source_break_child(&mut node["fields"]["parameter_two"], obj, true)?;
    if !obj.is_empty() {
        *obj = vec![super::new_source_node(
            "Math::Function::Underset",
            serde_json::json!({"parameter_one":null,"parameter_two":super::filter_source_values(std::mem::take(obj)),"options":{}}),
        )];
    }
    Ok(())
}

#[cfg(test)]
mod source_predicate_tests {
    #[test]
    fn source_prime_predicate_preserves_lexical_and_error_boundaries() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/final-writer-options/prime-source.json"
        ))
        .unwrap();
        for row in source["helpers"].as_array().unwrap() {
            let result = super::source_prime(&row["model"]);
            if let Some(expected) = row["prime"].as_bool() {
                assert_eq!(result.unwrap(), expected, "{}", row["id"]);
            } else {
                assert!(result.is_err(), "{}", row["id"]);
            }
        }
    }
}

#[cfg(test)]
mod source_symbol_predicates {
    use super::*;
    #[test]
    fn lazy_source_classes_preserve_prime_predicates() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../fixtures/final-writer-options/symbol-getters-source.json"
        ))
        .unwrap();
        let mut count = 0;
        for row in fixture["prime_checks"].as_array().unwrap() {
            let actual = source_prime(&row["model"]);
            if let Some(value) = row["value"].as_bool() {
                assert_eq!(actual.unwrap(), value, "{}", row["model"])
            } else {
                assert_eq!(row["error"]["class"], "NoMethodError");
                assert!(actual.unwrap_err().starts_with("source_render_error:"));
            }
            count += 1;
        }
        assert_eq!(count, 1459);
    }
}
