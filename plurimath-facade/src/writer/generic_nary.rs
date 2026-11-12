//! Generic Nary source procedures; distinct from Sum/Prod/Int/Oint.
use super::{
    attrs, control_properties, el, render_text, render_xml, run, text, Model, Value, WriterContext,
    Xml,
};

fn symbol_declarations() -> &'static Value {
    static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/generic-nary/symbol-declarations.json"
        ))
        .expect("source nary symbol declarations")
    })
}
fn truthy(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn model(v: &Value) -> Result<Model, String> {
    Model::from_value(v)
}
fn child(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let m = model(v)?;
    if m.class == "Math::Function::None" {
        return Ok(String::new());
    }
    render_text(&m, f, ctx)
}
pub(super) fn slot(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if f == "omml" && !truthy(v) {
        return Ok(vec![run("&#8203;")]);
    }
    if v.is_null() {
        return Ok(vec![]);
    }
    let values = if let Some(a) = v.as_array() {
        a.as_slice()
    } else {
        std::slice::from_ref(v)
    };
    let mut result = vec![];
    for value in values {
        let m = model(value)?;
        if m.class == "Math::Function::None" {
            result.push(if f == "mathml" {
                el("none", vec![])
            } else {
                run("&#8203;")
            });
        } else {
            result.extend(render_xml(&m, f, ctx)?);
        }
    }
    Ok(result)
}
fn mini(v: &Value) -> Result<bool, String> {
    let m = model(v)?;
    Ok(
        (m.class == "Math::Number" || m.class.starts_with("Math::Symbols::"))
            && (truthy(m.field("mini_sub_sized")) || truthy(m.field("mini_sup_sized"))),
    )
}
fn prime(v: &Value, ctx: &super::WriterContext) -> Result<bool, String> {
    if !v["class"]
        .as_str()
        .is_some_and(|c| c.starts_with("Math::Symbols::"))
    {
        return Ok(false);
    }
    if v["fields"]["value"]
        .as_str()
        .is_some_and(|s| s.contains("&#x27;"))
    {
        return Ok(true);
    }
    let s = child(v, "unicodemath", ctx)?;
    Ok(["′", "″", "‴", "⁗"].iter().any(|p| s.contains(p)))
}
fn parens(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    let s = child(v, "unicodemath", ctx)?;
    Ok(
        if v["class"]
            .as_str()
            .is_some_and(|s| s.starts_with("Math::Function::Fenced"))
        {
            s
        } else {
            format!("({s})")
        },
    )
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    let d = m.field("parameter_four");
    match f {
        "asciimath" | "latex" => {
            // None's textual methods return nil: the source || default is based
            // on nil, rather than the rendered string being empty.
            let base = if a.is_null() || a["class"] == "Math::Function::None" {
                if f == "latex" {
                    "\\int".into()
                } else {
                    "int".into()
                }
            } else {
                child(a, f, ctx)?
            };
            let sub = if truthy(b) {
                if f == "latex" {
                    format!("_{{{}}}", child(b, f, ctx)?)
                } else {
                    format!("_({})", child(b, f, ctx)?)
                }
            } else {
                String::new()
            };
            let sup = if truthy(c) {
                if f == "latex" {
                    format!("^{{{}}}", child(c, f, ctx)?)
                } else {
                    format!("^({})", child(c, f, ctx)?)
                }
            } else {
                String::new()
            };
            let operand = if truthy(d) {
                format!(" {}", child(d, f, ctx)?)
            } else {
                String::new()
            };
            Ok(format!("{base}{sub}{sup}{operand}"))
        }
        "html" => Err("source_render_error: generic Nary has no to_html method".into()),
        "unicodemath" => {
            let base = if a.is_null() {
                String::new()
            } else {
                child(a, f, ctx)?
            };
            let sub = if !truthy(b) {
                String::new()
            } else if mini(b)? {
                child(b, f, ctx)?
            } else if b["class"] == "Math::Function::Base" {
                format!("_{}", child(b, f, ctx)?)
            } else {
                format!("_{}", parens(b, ctx)?)
            };
            let is_prime = prime(c, ctx)?;
            let sup = if !truthy(c) {
                String::new()
            } else if mini(c)? || is_prime {
                child(c, f, ctx)?
            } else if c["class"] == "Math::Function::Power"
                || (a["class"] == "Math::Function::Power" && prime(&a["fields"]["parameter_two"], ctx)?)
            {
                format!("^{}", child(c, f, ctx)?)
            } else {
                format!("^{}", parens(c, ctx)?)
            };
            let operand = if !truthy(d) {
                String::new()
            } else if d["class"]
                .as_str()
                .is_some_and(|s| s.starts_with("Math::Function::Fenced"))
            {
                format!("▒{}", child(d, f, ctx)?)
            } else {
                format!("▒〖{}〗", child(d, f, ctx)?)
            };
            Ok(if is_prime {
                format!("{base}{sup}{sub}{operand}")
            } else {
                format!("{base}{sub}{sup}{operand}")
            })
        }
        _ => Err(format!("unported generic Nary text format {f}")),
    }
}
fn local_options(m: &Model) -> Result<serde_json::Map<String, Value>, String> {
    if !matches!(
        crate::source_values::field(m, "options", crate::source_values::Domain::NamedOptions)?,
        crate::source_values::RubyValue::Hash(_)
    ) {
        return Err("source_render_error: generic Nary options have no [] method".into());
    }
    crate::source_values::option_map(m, "options")
}
fn value_attr(name: &str, value: String) -> Xml {
    attrs(name, vec![("m:val".into(), value)], vec![])
}
fn type_string(v: &Value) -> Result<String, String> {
    match v {
        Value::Null | Value::Bool(false) => Ok("subSup".into()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(v) => Ok(v.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Array(a) if a.is_empty() => Ok("[]".into()),
        Value::Object(o) if o.is_empty() => Ok("{}".into()),
        _ => Err("unported compound generic Nary type.to_s value".into()),
    }
}
fn operator_char(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    if v.is_null() {
        return Ok(String::new());
    }
    let m = model(v)?;
    if m.kind() == "Formula" {
        let first =
            m.field("value").as_array().and_then(|v| v.first()).ok_or(
                "source_render_error: empty Formula has no first nary_attr_value receiver",
            )?;
        return operator_char(first, ctx);
    }
    if m.class == "Math::Number" {
        return match m.field("value") {
            Value::String(s) => Ok(s.clone()),
            Value::Null => Ok(String::new()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            _ => Err("unported Number nary_attr_value compound value".into()),
        };
    }
    if m.class == "Math::Function::Sum" {
        return Ok("∑".into());
    }
    if m.class == "Math::Function::Prod" {
        return Ok("∏".into());
    }
    if !m.class.starts_with("Math::Symbols::") {
        return Ok(String::new());
    }
    match m.field("value") {
        Value::String(s) => super::text_content::decode(s),
        Value::Null => {
            if let Some(glyph) =
                symbol_declarations()["classes"][m.class.as_str()]["nary_attribute"].as_str()
            {
                Ok(glyph.into())
            } else {
                child(v, "unicodemath", ctx)
            }
        }
        _ => Err("source_render_error: nary_attr_value has no include? method".into()),
    }
}
fn mathml_mask(
    name: &mut String,
    children: &mut Vec<Xml>,
    mask: &Value,
    m: &Model,
) -> Result<(), String> {
    // Reuse the existing source Core#get_mask_options scalar conversion.
    let mask = super::nary::mask_mod(mask)?;
    let extras = mask & 28;
    if extras & 16 != 0 && m.field("parameter_three").is_null() {
        *name = if name == "msub" {
            "msubsup"
        } else {
            "munderover"
        }
        .into();
        if children.len() < 2 {
            return Err("source_render_error: upper placeholder inserts nil XML child".into());
        }
        children.insert(2, el("mo", vec![text("&#x2b1a;")]));
    }
    if extras & 8 != 0 && m.field("parameter_two").is_null() {
        *name = if name == "msup" {
            "msubsup"
        } else {
            "munderover"
        }
        .into();
        if children.is_empty() {
            return Err("source_render_error: lower placeholder inserts nil XML child".into());
        }
        children.insert(1, el("mo", vec![text("&#x2b1a;")]));
    }
    if extras & 4 != 0 && ["munderover", "msubsup"].contains(&name.as_str()) {
        if children.len() < 3 {
            return Err("source_render_error: opposite limits select missing XML child".into());
        }
        *children = [0, 2, 1]
            .iter()
            .filter_map(|i| children.get(*i).cloned())
            .collect();
    }
    match mask % 4 {
        1 => {
            *name = match name.as_str() {
                "msubsup" => "munderover",
                "msub" => "munder",
                "msup" => "mover",
                _ => name.as_str(),
            }
            .into()
        }
        2 => {
            *name = match name.as_str() {
                "munderover" => "msubsup",
                "munder" => "msub",
                "mover" => "msup",
                _ => name.as_str(),
            }
            .into()
        }
        3 => {
            let _second = children.get(1).ok_or(
                "source_render_error: mask upper_limit_as_super_script reads missing XML node name",
            )?;
            // Core#masked_tag may construct a new node in this branch. Nary
            // ignores that return; the source builder tolerates a nil third
            // input, so no replacement or extra cardinality guard belongs here.
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    let c = m.field("parameter_three");
    let d = m.field("parameter_four");
    let options = local_options(m)?;
    if f == "omml" {
        let glyph = operator_char(a, ctx)?;
        let mut properties = vec![];
        if glyph != "∫" {
            properties.push(value_attr("m:chr", glyph));
        }
        properties.push(value_attr(
            "m:limLoc",
            if m.profile() == crate::source_values::Profile::RubyValuesV1 {
                let source = crate::source_values::field(
                    m,
                    "options",
                    crate::source_values::Domain::NamedOptions,
                )?;
                match source.get_symbol("type") {
                    None
                    | Some(crate::source_values::RubyValue::Nil)
                    | Some(crate::source_values::RubyValue::Bool(false)) => "subSup".into(),
                    Some(value) => crate::source_values::to_s(value)?,
                }
            } else {
                type_string(options.get("type").unwrap_or(&Value::Null))?
            },
        ));
        if b.is_null() {
            properties.push(value_attr("m:subHide", "1".into()));
        }
        if c.is_null() {
            properties.push(value_attr("m:supHide", "1".into()));
        }
        properties.push(control_properties());
        return Ok(vec![el(
            "m:nary",
            vec![
                el("m:naryPr", properties),
                el("m:sub", slot(b, f, ctx)?),
                el("m:sup", slot(c, f, ctx)?),
                el("m:e", slot(d, f, ctx)?),
            ],
        )]);
    }
    if f != "mathml" {
        return Err(format!("unported generic Nary XML format {f}"));
    }
    let over = options.get("type").is_some_and(|v| v == "undOvr");
    let mut name = match (truthy(b), truthy(c), over) {
        (true, true, true) => "munderover",
        (true, true, false) => "msubsup",
        (true, false, true) => "munder",
        (true, false, false) => "msub",
        (false, true, true) => "mover",
        (false, true, false) => "msup",
        _ => "mrow",
    }
    .to_string();
    let mut children = vec![];
    for v in [a, b, c] {
        children.extend(slot(v, f, ctx)?);
    }
    if let Some(mask) = options.get("mask").filter(|v| truthy(v)) {
        mathml_mask(&mut name, &mut children, mask, m)?;
    }
    let operator = el(&name, children);
    if !truthy(d) {
        return Ok(vec![operator]);
    }
    // wrap_mrow is called with true even when the outer intent is false.
    // It reads .name on the returned object, so an Array operand refuses.
    // intent_name is evaluated before intentify can return for intent=false.
    if a.is_array() {
        return Err(
            "source_render_error: generic Nary operator Array has no is_nary_symbol? method".into(),
        );
    }
    if d.is_array() {
        return Err("source_render_error: generic Nary array operand has no XML name".into());
    }
    let mut operand = slot(d, f, ctx)?;
    if operand.len() != 1 {
        return Err("source_render_error: generic Nary operand has no single XML name".into());
    }
    let operand = operand.remove(0);
    let operand = if matches!(&operand,Xml::El(n,_,_)if n=="mrow") {
        operand
    } else {
        el("mrow", vec![operand])
    };
    let mut result = el("mrow", vec![operator, operand]);
    if ctx.intent {
        apply_nary_intent(m, &mut result)?;
    }
    Ok(vec![result])
}

fn optional_intent_value(value: Option<&mut Xml>, name: &str) -> Result<String, String> {
    value
        .map(|v| super::intent_node_value(v, name))
        .unwrap_or_else(|| Ok(String::new()))
}
fn nary_bounds(operator: &mut Xml) -> Result<(String, String), String> {
    let tag = super::xml_name(operator).to_owned();
    let children = super::xml_nodes_mut(operator)?;
    match tag.as_str() {
        "msubsup" | "munderover" => {
            let low = optional_intent_value(children.get_mut(1), "l")?;
            let high = optional_intent_value(children.get_mut(2), "h")?;
            Ok((low, high))
        }
        "msub" | "munder" => Ok((
            optional_intent_value(children.get_mut(1), "l")?,
            String::new(),
        )),
        "msup" | "mover" => Ok((
            String::new(),
            optional_intent_value(children.get_mut(1), "h")?,
        )),
        _ => Ok((String::new(), String::new())),
    }
}
fn apply_nary_intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    let class = m.field("parameter_one")["class"].as_str().unwrap_or("");
    let name = symbol_declarations()["classes"][class]["intent_name"]
        .as_str()
        .unwrap_or(":n-ary");
    named_nary_intent(node, name)
}
fn family_declarations() -> &'static Value {
    static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../fixtures/nary-family/intent-declarations.json"
        ))
        .expect("source family intent names")
    })
}
pub(super) fn family_intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    let name = family_declarations()["classes"][m.class.as_str()]
        .as_str()
        .ok_or("unported nary family intent declaration")?;
    named_nary_intent(node, name)
}
fn named_nary_intent(node: &mut Xml, name: &str) -> Result<(), String> {
    if super::xml_name(node) != "mrow" {
        let (low, high) = nary_bounds(node)?;
        super::xml_attribute(node, "intent", format!("{name}({low},{high})"));
        return Ok(());
    }
    let children = super::xml_nodes_mut(node)?;
    let operator = children
        .first_mut()
        .ok_or("source_render_error: Nary intent has no operator")?;
    let (low, high) = nary_bounds(operator)?;
    let operand = optional_intent_value(children.get_mut(1), "naryand")?;
    super::xml_attribute(node, "intent", format!("{name}({low},{high},{operand})"));
    Ok(())
}

/// Nary#line_breaking visits operator, lower limit, then operand; it
/// deliberately skips the upper limit and preserves the options hash.
pub(super) fn line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    super::source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
    if !obj.is_empty() {
        let tail = super::filter_source_values(std::mem::take(obj));
        *obj = vec![super::new_source_node(
            "Math::Function::Nary",
            serde_json::json!({"parameter_one":tail,"parameter_two":node["fields"]["parameter_two"],"parameter_three":node["fields"]["parameter_three"],"parameter_four":node["fields"]["parameter_four"],"options":node["fields"]["options"]}),
        )];
        for field in ["parameter_two", "parameter_three", "parameter_four"] {
            node["fields"][field] = Value::Null;
        }
        return Ok(());
    }
    super::source_break_child(&mut node["fields"]["parameter_two"], obj, true)?;
    if !obj.is_empty() {
        let tail = super::filter_source_values(std::mem::take(obj));
        *obj = vec![super::new_source_node(
            "Math::Function::Nary",
            serde_json::json!({"parameter_one":null,"parameter_two":tail,"parameter_three":node["fields"]["parameter_three"],"parameter_four":node["fields"]["parameter_four"],"options":node["fields"]["options"]}),
        )];
        node["fields"]["parameter_three"] = Value::Null;
        node["fields"]["parameter_four"] = Value::Null;
        return Ok(());
    }
    super::source_break_child(&mut node["fields"]["parameter_four"], obj, true)?;
    if !obj.is_empty() {
        *obj = vec![super::filter_source_values(std::mem::take(obj))];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_decimal_prefix_mask_casts() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/generic-nary/mask-cast-source.json"
        ))
        .unwrap();
        for row in data["rows"].as_array().unwrap() {
            let actual = super::super::nary::mask_mod(&row["input"]).unwrap();
            assert_eq!(
                actual as u64,
                row["mod32"].as_u64().unwrap(),
                "{}",
                row["input"]
            );
        }
    }
}
