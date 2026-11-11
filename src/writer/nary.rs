//! Source procedures for Sum, Prod, Int and Oint (Ruby 0a1df68a).
//! Catalog constants are measured by tests/support/export-nary.rb; no fixture lookup.
use super::{attrs, child, el, strict_slot_xml, text as xml_text, WriterContext, Xml};
use crate::model::Model;
use serde_json::Value;
use std::sync::OnceLock;

fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("nary-data.json")).unwrap())
}
fn constant<'a>(m: &Model, field: &str) -> &'a str {
    data()["classes"][m.kind()][field].as_str().unwrap()
}
pub(super) fn handles(m: &Model) -> bool {
    matches!(
        m.class.as_str(),
        "Math::Function::Sum"
            | "Math::Function::Prod"
            | "Math::Function::Int"
            | "Math::Function::Oint"
    )
}
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn source_error(message: &str) -> String {
    format!("source_render_error: {message}")
}
// Number's five non-Unicode source writers ignore mini_sub_sized/mini_sup_sized.
// Normalize only a private rendering copy; the common model wire stays unchanged.
fn ordinary_numbers(v: &Value) -> Value {
    let mut value = v.clone();
    fn visit(v: &mut Value) {
        if let Some(a) = v.as_array_mut() {
            for v in a {
                visit(v);
            }
            return;
        }
        if !v.is_object() {
            return;
        }
        let number = v.get("class").and_then(Value::as_str) == Some("Math::Number");
        if let Some(fields) = v.get_mut("fields").and_then(Value::as_object_mut) {
            if number {
                fields.insert("mini_sub_sized".into(), Value::Bool(false));
                fields.insert("mini_sup_sized".into(), Value::Bool(false));
            }
            for value in fields.values_mut() {
                visit(value);
            }
        }
    }
    visit(&mut value);
    value
}
fn node_text(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if v.is_null() {
        return Ok(String::new());
    }
    if !v.is_object() {
        return Err(source_error("n-ary text argument has no source writer"));
    }
    if f == "unicodemath" {
        child(v, f, ctx)
    } else {
        child(&ordinary_numbers(v), f, ctx)
    }
}
fn mini_sized(v: &Value) -> bool {
    if !v.is_object() {
        return false;
    }
    let Ok(m) = Model::from_value(v) else {
        return false;
    };
    match m.kind() {
        "Number" => truthy(m.field("mini_sub_sized")) || truthy(m.field("mini_sup_sized")),
        "Formula" => m
            .field("value")
            .as_array()
            .and_then(|a| a.first())
            .map(mini_sized)
            .unwrap_or(false),
        _ => false,
    }
}
fn contains_mini(v: &Value) -> bool {
    if let Some(a) = v.as_array() {
        return a.iter().any(contains_mini);
    }
    if !v.is_object() {
        return false;
    }
    if mini_sized(v) {
        return true;
    }
    v.get("fields")
        .and_then(Value::as_object)
        .map(|o| o.values().any(contains_mini))
        .unwrap_or(false)
}
fn unicode_child(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    if !v.is_object() {
        return Err(source_error("n-ary Unicode argument has no source writer"));
    }
    let m = Model::from_value(v)?;
    if m.kind() == "Number" && mini_sized(v) {
        let map = if truthy(m.field("mini_sub_sized")) {
            "mini_sub"
        } else {
            "mini_sup"
        };
        let value = m
            .field("value")
            .as_str()
            .ok_or_else(|| "unported n-ary mini number non-string value".to_owned())?;
        // Number#mini_sub/#mini_sup return nil when the complete value is not in the digit map.
        return Ok(data()[map][value].as_str().unwrap_or("").to_owned());
    }
    if m.kind() == "Formula" {
        let values = m
            .field("value")
            .as_array()
            .ok_or_else(|| source_error("Formula Unicode value is not an array"))?;
        let negated = values
            .last()
            .map(|last| {
                data()["prime_symbol_classes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|class| last.get("class") == Some(class))
                    && last
                        .get("fields")
                        .and_then(|f| f.get("value"))
                        .and_then(Value::as_str)
                        == Some("&#x338;")
            })
            .unwrap_or(false);
        let separator = if mini_sized(v) || negated { "" } else { " " };
        let pieces = values
            .iter()
            .map(|value| unicode_child(value, ctx))
            .collect::<Result<Vec<_>, _>>()?;
        return super::text_content::decode(&pieces.join(separator));
    }
    if contains_mini(v) && !handles(&m) {
        return Err("unported promised nested mini-number Unicode child writer".into());
    }
    node_text(v, "unicodemath", ctx)
}
fn parens(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let value = if f == "unicodemath" {
        unicode_child(v, ctx)?
    } else {
        node_text(v, f, ctx)?
    };
    let fenced = v.get("class").and_then(Value::as_str) == Some("Math::Function::Fenced");
    if f == "unicodemath" && fenced {
        Ok(value)
    } else {
        Ok(format!("({value})"))
    }
}
fn prime(v: &Value) -> Result<bool, String> {
    let class = v.get("class").and_then(Value::as_str).unwrap_or("");
    if !class.starts_with("Math::Symbols::") {
        return Ok(false);
    }
    let value = v
        .get("fields")
        .and_then(|o| o.get("value"))
        .and_then(Value::as_str)
        .ok_or_else(|| source_error("prime predicate requires a symbol value"))?;
    if value.contains("&#x27;") {
        return Ok(true);
    }
    let input = if class == "Math::Symbols::Symbol" {
        value
    } else {
        data()["prime_inputs"][class]
            .as_str()
            .ok_or_else(|| "unported promised typed-symbol prime predicate".to_owned())?
    };
    Ok(data()["primes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| input.contains(p.as_str().unwrap())))
}
fn unicode_bound(m: &Model, upper: bool, ctx: &super::WriterContext) -> Result<String, String> {
    let v = m.field(if upper {
        "parameter_two"
    } else {
        "parameter_one"
    });
    if !truthy(v) {
        return Ok(String::new());
    }
    let sign = if upper { "^" } else { "_" };
    if matches!(m.kind(), "Prod" | "Int") {
        if mini_sized(v) || (upper && prime(v)?) {
            return unicode_child(v, ctx);
        }
        let special = if upper {
            "Math::Function::Power"
        } else {
            "Math::Function::Base"
        };
        if v.get("class").and_then(Value::as_str) == Some(special) {
            return Ok(format!("{sign}{}", unicode_child(v, ctx)?));
        }
    }
    Ok(format!("{sign}{}", parens(v, "unicodemath", ctx)?))
}
fn node_mask(m: &Model) -> Result<Option<crate::source_values::RubyValue>, String> {
    use crate::source_values::{self as sv, Domain, RubyValue};
    let options = sv::field(m, "options", Domain::NamedOptions)?;
    if matches!(options, RubyValue::Nil) {
        return Ok(None);
    }
    if !matches!(options, RubyValue::Hash(_)) {
        return Err(source_error("n-ary options have no key? method"));
    }
    Ok(options.get_symbol("mask").cloned())
}

fn ruby_mask_text(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) if !n.to_string().contains(['.', 'e', 'E']) => Ok(n.to_string()),
        _ => Err("unported promised n-ary non-integer mask string spelling".into()),
    }
}
fn latex_wrapped(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    let value = node_text(v, "latex", ctx)?;
    let m = Model::from_value(v)?;
    let function_formula = if m.kind() == "Formula" {
        let values = m
            .field("value")
            .as_array()
            .ok_or_else(|| source_error("Formula predicate has no value array"))?;
        let has = |classes: &Value| {
            values.iter().any(|v| {
                classes
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|class| v.get("class") == Some(class))
            })
        };
        !has(&data()["left_classes"]) || !has(&data()["right_classes"])
    } else {
        data()["latex_wrap_static"][&m.class]
            .as_bool()
            .ok_or_else(|| {
                "unported promised value-sensitive LaTeX wrapping predicate".to_owned()
            })?
    };
    if function_formula {
        Ok(format!("{{ \\left ( {value} \\right ) }}"))
    } else {
        Ok(format!("{{{value}}}"))
    }
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    match f {
        "asciimath" | "latex" => {
            let mut result = constant(m, if f == "latex" { "latex" } else { "ascii" }).to_owned();
            for (v, mark) in [(a, "_"), (b, "^")] {
                if truthy(v) {
                    result.push_str(mark);
                    if f == "asciimath" {
                        result.push_str(&parens(v, f, ctx)?);
                    } else if m.kind() == "Oint" {
                        result.push_str(&latex_wrapped(v, ctx)?);
                    } else {
                        result.push_str(&format!("{{{}}}", node_text(v, f, ctx)?));
                    }
                }
            }
            result.push(' ');
            if !c.is_null() {
                result.push_str(&node_text(c, f, ctx)?);
            }
            Ok(result
                .trim_matches(|c: char| c == '\0' || c.is_ascii_whitespace())
                .to_owned())
        }
        "unicodemath" => {
            let mut result = constant(m, "unicode").to_owned();
            // Prod uses the caller's options, not its own options. Non-default caller
            // options remain refused by the parent writer until its context is ported.
            if m.kind() != "Prod" {
                if let Some(mask) = node_mask(m)? {
                    result.push_str(&crate::source_values::to_s(&mask)?);
                }
            }
            result.push_str(&unicode_bound(m, false, ctx)?);
            result.push_str(&unicode_bound(m, true, ctx)?);
            if truthy(c) {
                let body = unicode_child(c, ctx)?;
                result.push('▒');
                if c.get("class").and_then(Value::as_str) == Some("Math::Function::Fenced") {
                    result.push_str(&body);
                } else {
                    result.push('〖');
                    result.push_str(&body);
                    result.push('〗');
                }
            }
            Ok(result)
        }
        "html" => {
            let mut result = constant(m, "html_operator").to_owned();
            if matches!(m.kind(), "Sum" | "Prod") {
                if truthy(a) {
                    result.push_str(&format!("<sub>{}</sub>", node_text(a, f, ctx)?));
                }
                if truthy(b) {
                    result.push_str(&format!("<sup>{}</sup>", node_text(b, f, ctx)?));
                }
            } else {
                for v in [a, b, c] {
                    if truthy(v) {
                        result.push_str(&format!("<i>{}</i>", node_text(v, f, ctx)?));
                    }
                }
            }
            Ok(result)
        }
        _ => Err(format!("unknown n-ary text format {f}")),
    }
}
fn math_child(v: &Value, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if v["class"] == "Math::Function::None" {
        return super::generic_nary::slot(v, "mathml", ctx);
    }
    strict_slot_xml(&ordinary_numbers(v), "mathml", ctx, false)
}
fn omml_child(v: &Value, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if let Some(a) = v.as_array() {
        let mut out = vec![];
        for item in a {
            if !item.is_object() {
                return Err(source_error("n-ary OMML array item has no insert_t_tag"));
            }
            out.extend(strict_slot_xml(
                &ordinary_numbers(item),
                "omml",
                ctx,
                false,
            )?);
        }
        Ok(out)
    } else {
        strict_slot_xml(&ordinary_numbers(v), "omml", ctx, false)
    }
}
fn omml_parameter(v: &Value, name: &str, ctx: &WriterContext) -> Result<Xml, String> {
    let children = if truthy(v) {
        omml_child(v, ctx)?
    } else {
        vec![el("m:r", vec![el("m:t", vec![xml_text("&#8203;")])])]
    };
    Ok(el(name, children))
}
fn val(name: &str, value: &str) -> Xml {
    attrs(name, vec![("m:val".into(), value.into())], vec![])
}
fn omml(m: &Model, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    if ![a, b, c].iter().any(|v| truthy(v)) {
        return Ok(vec![el(
            "m:r",
            vec![el("m:t", vec![xml_text(constant(m, "omml_operator_text"))])],
        )]);
    }
    let operator = if truthy(m.field("hide_function_name")) {
        ""
    } else {
        constant(m, "unicode")
    };
    let properties = el(
        "m:naryPr",
        vec![
            val("m:chr", operator),
            val("m:limLoc", constant(m, "lim_location")),
            val("m:subHide", if truthy(a) { "0" } else { "1" }),
            val("m:supHide", if truthy(b) { "0" } else { "1" }),
        ],
    );
    Ok(vec![el(
        "m:nary",
        vec![
            properties,
            omml_parameter(a, "m:sub", ctx)?,
            omml_parameter(b, "m:sup", ctx)?,
            omml_parameter(c, "m:e", ctx)?,
        ],
    )])
}
// Ruby Integer/String#to_i modulo32 is enough for Core#get_mask_options.
fn integer_mask_mod(text: &str) -> u8 {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let negative = bytes.get(i) == Some(&b'-');
    if matches!(bytes.get(i), Some(b'+') | Some(b'-')) {
        i += 1;
    }
    // Ruby String#to_i(base10) recognizes only the explicit decimal
    // prefix here; 0x/0b/0o still stop at the initial zero.
    if bytes.get(i) == Some(&b'0') && matches!(bytes.get(i + 1), Some(b'd') | Some(b'D')) {
        i += 2;
    }
    let mut n = 0u8;
    let mut digit = false;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            n = ((u16::from(n) * 10 + u16::from(bytes[i] - b'0')) % 32) as u8;
            digit = true;
            i += 1;
        } else if bytes[i] == b'_'
            && digit
            && bytes
                .get(i + 1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            i += 1;
        } else {
            break;
        }
    }
    if negative {
        (32 - n) % 32
    } else {
        n
    }
}
pub(super) fn mask_mod(v: &Value) -> Result<u8, String> {
    match v {
        Value::Null => Ok(0),
        Value::String(s) => Ok(integer_mask_mod(s)),
        Value::Number(n) => {
            let s = n.to_string();
            if !s.contains(['.', 'e', 'E']) {
                return Ok(integer_mask_mod(&s));
            }
            let f = n
                .as_f64()
                .ok_or_else(|| source_error("mask has no finite to_i"))?;
            if !f.is_finite() {
                return Err(source_error("mask has no finite to_i"));
            }
            let bits = f.abs().to_bits();
            let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
            let n = if exponent < 0 {
                0
            } else if exponent >= 57 {
                0
            } else {
                let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
                if exponent >= 52 {
                    ((mantissa << (exponent - 52)) & 31) as u8
                } else {
                    ((mantissa >> (52 - exponent)) & 31) as u8
                }
            };
            Ok(if f.is_sign_negative() {
                (32 - n) % 32
            } else {
                n
            })
        }
        _ => Err(source_error("mask has no to_i method")),
    }
}
pub(super) fn source_mask_mod(value: &crate::source_values::RubyValue) -> Result<u8, String> {
    use crate::source_values::RubyValue;
    match value {
        RubyValue::Nil => Ok(0),
        RubyValue::Integer(v) | RubyValue::String(v) => Ok(integer_mask_mod(v)),
        RubyValue::Float(bits) => {
            let value = f64::from_bits(*bits);
            if !value.is_finite() {
                return Err(source_error("mask has no finite to_i"));
            }
            mask_mod(&serde_json::json!(value))
        }
        _ => Err(source_error("mask has no to_i method")),
    }
}
fn element_parts(node: Xml) -> Result<(String, Vec<(String, String)>, Vec<Xml>), String> {
    if let Xml::El(n, a, c) = node {
        Ok((n, a, c))
    } else {
        Err("internal n-ary expected element".into())
    }
}
fn mask_mathml(
    node: Xml,
    mask: &crate::source_values::RubyValue,
    m: &Model,
) -> Result<Xml, String> {
    let mask = source_mask_mod(mask)?;
    let (n, attrs, mut children) = element_parts(node)?;
    let mut name = n;
    let extras = mask & 28;
    if extras & 16 != 0 && m.field("parameter_three").is_null() {
        name = if name == "msub" {
            "msubsup"
        } else {
            "munderover"
        }
        .into();
        children.insert(2.min(children.len()), el("mo", vec![xml_text("&#x2b1a;")]));
    }
    if extras & 8 != 0 && m.field("parameter_two").is_null() {
        name = if name == "msup" {
            "msubsup"
        } else {
            "munderover"
        }
        .into();
        children.insert(1.min(children.len()), el("mo", vec![xml_text("&#x2b1a;")]));
    }
    if extras & 4 != 0 && matches!(name.as_str(), "munderover" | "msubsup") {
        children = [0, 2, 1]
            .iter()
            .filter_map(|i| children.get(*i).cloned())
            .collect();
    }
    match mask % 4 {
        1 => {
            name = match name.as_str() {
                "msubsup" => "munderover",
                "msub" => "munder",
                "msup" => "mover",
                _ => &name,
            }
            .into();
        }
        2 => {
            name = match name.as_str() {
                "munderover" => "msubsup",
                "munder" => "msub",
                "mover" => "msup",
                _ => &name,
            }
            .into();
        }
        3 => {
            if matches!(children.get(1),Some(Xml::El(n,_,_))if n=="mrow") {
                let upper = children.remove(2);
                let lower = children.remove(1);
                let base = children.remove(0);
                return Ok(el("munder", vec![el("msup", vec![base, upper]), lower]));
            }
        }
        _ => {}
    }
    Ok(Xml::El(name, attrs, children))
}
fn mathml(m: &Model, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    let hide = m.kind() != "Int" && truthy(m.field("hide_function_name"));
    let operator = el(
        "mo",
        if hide {
            vec![]
        } else {
            vec![xml_text(constant(m, "mathml_operator"))]
        },
    );
    if ![a, b, c].iter().any(|v| truthy(v)) {
        return Ok(vec![operator]);
    }
    let suffix = if truthy(a) && truthy(b) {
        "subsup"
    } else if truthy(a) {
        "sub"
    } else {
        "sup"
    };
    let tag = match m.kind() {
        "Int" => "msubsup".to_owned(),
        "Oint" => format!("m{suffix}"),
        _ => format!(
            "m{}",
            match suffix {
                "subsup" => "underover",
                "sub" => "under",
                _ => "over",
            }
        ),
    };
    let mut args = vec![operator];
    for v in [a, b] {
        if m.kind() == "Int" && !truthy(v) {
            args.push(el("mrow", vec![]));
        } else {
            args.extend(math_child(v, ctx)?);
        }
    }
    let mut bounds = el(&tag, args);
    if m.kind() == "Int" {
        if let Some(mask) = node_mask(m)? {
            bounds = mask_mathml(bounds, &mask, m)?;
        }
    }
    let no_body = if m.kind() == "Oint" {
        !truthy(c)
    } else {
        c.is_null()
    };
    if no_body {
        // Sum and Int return before intentify; Prod and Oint call it even
        // without an operand. This difference is source-visible.
        if ctx.intent && matches!(m.kind(), "Prod" | "Oint") {
            super::generic_nary::family_intent(m, &mut bounds)?;
        }
        return Ok(vec![bounds]);
    }
    let mut row = vec![bounds];
    let mut operand = math_child(c, ctx)?;
    if ctx.intent {
        if operand.len() != 1 {
            return Err("source_render_error: nary operand has no single XML name".into());
        }
        let operand = operand.remove(0);
        row.push(if matches!(&operand,Xml::El(n,_,_)if n=="mrow") {
            operand
        } else {
            el("mrow", vec![operand])
        });
    } else {
        row.extend(operand);
    }
    let mut result = el("mrow", row);
    if ctx.intent {
        super::generic_nary::family_intent(m, &mut result)?;
    }
    Ok(vec![result])
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    match f {
        "mathml" => mathml(m, ctx),
        "omml" => omml(m, ctx),
        _ => Err(format!("unknown n-ary XML format {f}")),
    }
}

/// Source overrides skip the upper limit. Prod deliberately retains its
/// original upper/operand fields after a lower-limit continuation.
pub(super) fn line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    let m = Model::from_value(node)?;
    super::source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
    if !obj.is_empty() {
        let lower = super::filter_source_values(std::mem::take(obj));
        let continuation = super::new_source_node(
            &m.class,
            serde_json::json!({"parameter_one":lower,"parameter_two":node["fields"]["parameter_two"],"parameter_three":node["fields"]["parameter_three"],"hide_function_name":true}),
        );
        *obj = vec![continuation];
        if m.kind() != "Prod" {
            node["fields"]["parameter_two"] = Value::Null;
            node["fields"]["parameter_three"] = Value::Null;
        }
        return Ok(());
    }
    super::source_break_child(&mut node["fields"]["parameter_three"], obj, true)?;
    if !obj.is_empty() {
        *obj = vec![super::filter_source_values(std::mem::take(obj))];
    }
    Ok(())
}
