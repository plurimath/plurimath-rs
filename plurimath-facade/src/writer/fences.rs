//! Ruby Fenced procedures (source 0a1df68a), independent of observed input values.
use super::{attrs, child, el, strict_slot_xml, text as xml_text, WriterContext, Xml};
use crate::model::Model;
use serde_json::{json, Value};

fn source_error(message: &str) -> String {
    format!("source_render_error: {message}")
}
fn present(v: &Value) -> bool {
    !v.is_null() && v != false
}
pub(super) fn handles(m: &Model) -> bool {
    m.class == "Math::Function::Fenced"
}
/// Fenced overrides Core traversal. It splits only direct Linebreak children,
/// clears the original closing delimiter and the continuation's opening delimiter.
/// Later linebreaks remain in that clone for the next pass.
pub(super) fn line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    let Some(values) = node["fields"]["parameter_two"].as_array() else {
        return Ok(());
    };
    let Some(index) = super::linebreak_index(values) else {
        return Ok(());
    };
    let mut continuation = node.clone();
    let mut head = values[..index].to_vec();
    let mut tail = values[index + 1..].to_vec();
    let marker = &values[index];
    let parameter = &marker["fields"]["parameter_one"];
    // UnaryFunction#exist? distinguishes nil/empty Array from false.
    if !parameter.is_null() && !parameter.as_array().is_some_and(Vec::is_empty) {
        let attributes = &marker["fields"]["attributes"];
        if !attributes.is_object() {
            return Err(source_error("Linebreak attributes have no [] method"));
        }
        if attributes["linebreakstyle"] == "after" {
            head.push(parameter.clone())
        } else {
            tail.insert(0, parameter.clone())
        }
    }
    node["fields"]["parameter_two"] = Value::Array(head);
    node["fields"]["parameter_three"] = Value::Null;
    // value_split flattens the *remaining* result groups, not the first line.
    let mut flat = vec![];
    let mut pending = tail.into_iter().rev().collect::<Vec<_>>();
    while let Some(value) = pending.pop() {
        if let Value::Array(values) = value {
            pending.extend(values.into_iter().rev())
        } else {
            flat.push(value)
        }
    }
    continuation["fields"]["parameter_one"] = Value::Null;
    continuation["fields"]["parameter_two"] = Value::Array(flat);

    *obj = vec![continuation];
    Ok(())
}
fn list(m: &Model) -> Result<&[Value], String> {
    let value = m.field("parameter_two");
    if value.is_null() || value.as_object().is_some_and(|o| o.is_empty()) {
        return Ok(&[]);
    }
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| source_error("Fenced body has no map method"))
}
fn scalar(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) if !n.to_string().contains(['.', 'e', 'E']) => Ok(n.to_string()),
        _ => Err("unported promised Ruby fence scalar spelling".into()),
    }
}
fn delimiter(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<Option<String>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let m =
        Model::from_value(v).map_err(|_| source_error("Fenced delimiter has no value method"))?;
    if m.class.starts_with("Math::Symbols::Paren::") {
        return super::fence_delimiter(v, f, ctx).map(Some);
    }
    if m.field("value").is_null() {
        Ok(None)
    } else {
        scalar(m.field("value")).map(Some)
    }
}
fn choose(v: &Value) -> bool {
    if v["class"]!="Math::Function::Frac" {return false;}
    let Ok(model)=Model::from_value(v)else{return false};
    crate::source_values::field(&model,"options",crate::source_values::Domain::NamedOptions).ok().is_some_and(|options|options.get_symbol("choose").is_some())
}

fn choose_truthy(v: &Value) -> Result<bool, String> {
    use crate::source_values::{field, Domain, RubyValue};
    let options=field(&Model::from_value(v)?,"options",Domain::NamedOptions)?;
    Ok(options.get_symbol("choose").is_some_and(|value|!matches!(value,RubyValue::Nil|RubyValue::Bool(false))))
}

fn child_model(v: &Value) -> Result<Value, String> {
    if !v.is_object() {
        return Err(source_error("Fenced text child is not a node"));
    }
    let v = v.clone();
    Ok(v)
}
pub(super) fn symbol_unicode(v: &Value) -> Result<Option<String>, String> {
    if v["class"] != "Math::Symbols::Symbol" {return Ok(None);}

        let fields = &v["fields"];
        let value = fields["value"].as_str();
        if present(&fields["slashed"]) || value.is_some_and(|s| ["&", "@", "^"].contains(&s)) {
            return Ok(Some(format!("\\{}", scalar(&fields["value"])?)));
        }
        let prefix = if present(&fields["mini_sub_sized"]) {
            Some("SUB")
        } else if present(&fields["mini_sup_sized"]) {
            Some("SUP")
        } else {
            None
        };
        if let Some(prefix) = prefix {
            let value =
                value.ok_or_else(|| source_error("Mini Symbol value has no to_sym method"))?;
            static DATA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
            let data = DATA.get_or_init(|| {
                serde_json::from_str(include_str!("fences-data.json"))
                    .expect("sealed source glyph constants")
            });
            let data = &data["mini_constants"];
            for suffix in ["ALPHABETS", "OPERATORS"] {
                if let Some(s) = data[format!("{prefix}_{suffix}")][value].as_str() {
                    return Ok(Some(s.into()));
                }
            }
            if let Some(groups) = data[format!("{prefix}_PARENTHESIS")].as_object() {
                for group in groups.values() {
                    if let Some(s) = group[value].as_str() {
                        return Ok(Some(s.into()));
                    }
                }
            }
            return Ok(Some(String::new()));
        }
    Ok(None)
}
fn ordinary_child(v: &Value, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    if f=="unicodemath" {if let Some(value)=symbol_unicode(v)? {return Ok(value);}}
    child(&child_model(v)?,f,ctx)
}

fn fraction_part(v: &Value, ctx: &super::WriterContext) -> Result<String, String> {
    if !present(v) {
        return Ok(String::new());
    }
    let s = ordinary_child(v, "unicodemath", ctx)?;
    Ok(if v["class"] == "Math::Function::Fenced" {
        s
    } else {
        format!("({s})")
    })
}
fn mini(v: &Value) -> bool {
    if !v.is_object() {
        return false;
    }
    if v["class"]
        .as_str()
        .is_some_and(|c| c.starts_with("Math::Symbols::"))
    {
        return present(&v["fields"]["mini_sub_sized"]) || present(&v["fields"]["mini_sup_sized"]);
    }
    match v["class"].as_str().unwrap_or("") {
        "Math::Number" | "Math::Symbols::Symbol" => {
            present(&v["fields"]["mini_sub_sized"]) || present(&v["fields"]["mini_sup_sized"])
        }
        "Math::Formula" => v["fields"]["value"]
            .as_array()
            .and_then(|a| a.first())
            .is_some_and(mini),
        "Math::Function::Fenced" => {
            mini(&v["fields"]["parameter_one"])
                || v["fields"]["parameter_two"]
                    .as_array()
                    .and_then(|a| a.first())
                    .is_some_and(mini)
                || mini(&v["fields"]["parameter_three"])
        }
        _ => false,
    }
}
/// Formula delegates its leading-mini predicate to the actual first node.
/// This adapter covers Fenced; other leading-node predicates remain owned by
/// the common Formula/Number source procedure.
pub(super) fn formula_starts_mini_fence(m: &Model) -> bool {
    m.field("value")
        .as_array()
        .and_then(|v| v.first())
        .is_some_and(|v| v["class"] == "Math::Function::Fenced" && mini(v))
}
fn size(v: &Value) -> Result<String, String> {
    let s = v
        .as_str()
        .ok_or_else(|| source_error("Fence minsize has no delete_suffix method"))?;
    let s = s
        .strip_suffix("em")
        .unwrap_or(s)
        .trim_start_matches([' ', '\t', '\n', '\r', '\u{000b}', '\u{000c}']);
    static PREFIX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let prefix=PREFIX.get_or_init(||regex::Regex::new(r"^[+-]?(?:[0-9](?:_?[0-9])*(?:\.(?:[0-9](?:_?[0-9])*)?)?|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?").unwrap());
    let n = prefix
        .find(s)
        .and_then(|v| v.as_str().replace('_', "").parse::<f64>().ok())
        .unwrap_or(0.0);
    let n = (n.ln() / 1.25_f64.ln()).round();
    if !n.is_finite() {
        return Err(source_error(
            "Fence minsize produces a non-finite logarithm",
        ));
    }
    Ok(if n == 0.0 {
        "0".into()
    } else {
        format!("{n:.0}")
    })
}
fn unicode_paren(m: &Model, opening: bool, ctx: &super::WriterContext) -> Result<String, String> {
    let field = m.field(if opening {
        "parameter_one"
    } else {
        "parameter_three"
    });
    let paren = if field.is_null() {
        String::new()
    } else {
        ordinary_child(field, "unicodemath", ctx)?
    };
    let options = m.field("options");
    if !options.is_null() && !options.is_object() {
        return Err(source_error("Fence options have no key? method"));
    }
    let key = if opening { "open_paren" } else { "close_paren" };
    let prefix = if opening { "├" } else { "┤" };
    if options.as_object().is_some_and(|o| o.contains_key(key)) {
        return Ok(format!(
            "{prefix}{}{paren}",
            size(&options[key]["minsize"])?
        ));
    }
    let prefixed = options.as_object().is_some_and(|o| {
        o.contains_key(if opening {
            "open_prefixed"
        } else {
            "close_prefixed"
        })
    });
    if prefixed {
        let value = &field["fields"]["value"];
        if !value.is_null() && !value.is_string() {
            return Err(source_error("Fence delimiter value has no include? method"));
        }
        let raw = field["fields"]["value"].as_str().unwrap_or("");
        let already = if opening {
            ["&#x251c;", "&#x3016;", "{:"]
        } else {
            ["&#x2524;", "&#x3017;", ":}"]
        }
        .iter()
        .any(|s| raw.contains(s));
        return Ok(if already {
            prefix.into()
        } else {
            format!("{prefix}{paren}")
        });
    }
    if paren == if opening { "{:" } else { ":}" } {
        Ok(prefix.into())
    } else {
        Ok(paren)
    }
}
pub(super) fn text(m: &Model, f: &str, ctx: &super::WriterContext) -> Result<String, String> {
    let body = if f == "html" && !present(m.field("parameter_two")) {
        &[]
    } else {
        list(m)?
    };
    let left = m.field("parameter_one");
    let right = m.field("parameter_three");
    if f == "unicodemath" {
        if !m.field("parameter_two").is_null() && !m.field("parameter_two").is_array() {
            return Err(source_error("Fenced body does not implement mini_sized?"));
        }
        let mini = mini(left) || body.first().is_some_and(mini) || mini(right);
        if mini {
            if !left.is_object() || !right.is_object() {
                return Err(source_error("Mini fence requires delimiter node writers"));
            }
            let b = body
                .iter()
                .map(|v| ordinary_child(v, f, ctx))
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            return Ok(format!(
                "{}{}{}",
                ordinary_child(left, f, ctx)?,
                b,
                ordinary_child(right, f, ctx)?
            ));
        }
        let b = body
            .iter()
            .map(|v| {
                if choose(v) {
                    Ok(format!(
                        "{}⒞{}",
                        fraction_part(&v["fields"]["parameter_one"], ctx)?,
                        fraction_part(&v["fields"]["parameter_two"], ctx)?
                    ))
                } else {
                    ordinary_child(v, f, ctx)
                }
            })
            .collect::<Result<Vec<_>, String>>()?
            .join(" ");
        if m.field("parameter_two").is_null() {
            return Err(source_error("Fenced nil body has no first method"));
        }
        if body.first().is_some_and(choose) {
            return Ok(b);
        }
        let l = Model::from_value(left)
            .map_err(|_| source_error("Fenced opening delimiter has no class_name method"))?;
        if l.kind() == "Symbol" && !l.field("value").is_null() && !l.field("value").is_string() {
            return Err(source_error("Symbol fence value has no include? method"));
        }
        let vertical = l.class == "Math::Symbols::Paren::Vert"
            || (l.kind() == "Symbol" && l.field("value").as_str().is_some_and(|v| v.contains('|')));
        return Ok(format!(
            "{}{}{}",
            unicode_paren(m, true, ctx)?,
            if vertical { format!("({b})") } else { b },
            unicode_paren(m, false, ctx)?
        ));
    }
    let b = body
        .iter()
        .map(|v| ordinary_child(v, f, ctx))
        .collect::<Result<Vec<_>, _>>()?
        .join(if f == "html" { "" } else { " " });
    match f {
        "asciimath" => Ok(format!(
            "{}{}{}",
            if present(left) {
                ordinary_child(left, f, ctx)?
            } else {
                "(".into()
            },
            b,
            if present(right) {
                ordinary_child(right, f, ctx)?
            } else {
                ")".into()
            }
        )),
        "html" => Ok(format!(
            "{}{}{}",
            if present(left) {
                format!("<i>{}</i>", delimiter(left, f, ctx)?.unwrap_or_default())
            } else {
                String::new()
            },
            b,
            if present(right) {
                format!("<i>{}</i>", delimiter(right, f, ctx)?.unwrap_or_default())
            } else {
                String::new()
            }
        )),
        "latex" => {
            let clean = |v: &Value| -> Result<String, String> {
                let s = delimiter(v, f, ctx)?.unwrap_or_default();
                Ok(if ["{:", ":}"].contains(&s.as_str()) {
                    s.replace(':', "")
                } else {
                    s
                })
            };
            Ok(format!("{} {b} {}", clean(left)?, clean(right)?))
        }
        _ => Err(format!("unknown fence text format {f}")),
    }
}
fn attributes(v: &Value) -> Result<Vec<(String, String)>, String> {
    if v.is_null() {
        return Ok(vec![]);
    }
    v.as_object()
        .ok_or_else(|| source_error("Fence attributes are not a hash"))?
        .iter()
        .map(|(k, v)| {
            Ok((
                k.clone(),
                if v.as_object().is_some_and(|o| o.is_empty()) {
                    "{}".into()
                } else if v.as_array().is_some_and(|a| a.is_empty()) {
                    "[]".into()
                } else {
                    scalar(v)?
                },
            ))
        })
        .collect()
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if f=="mathml" {
        if let Some(first)=m.field("parameter_two").as_array().and_then(|values|values.first()) {
            if first["class"]=="Math::Function::Frac" {
                use crate::source_values::{field,Domain,RubyValue};
                let options=field(&Model::from_value(first)?,"options",Domain::NamedOptions)?;
                if !matches!(options,RubyValue::Nil|RubyValue::Hash(_)) {
                    return Err("source_render_error: fenced fraction options have no key? method".into());
                }
            }
        }
    }

    xml_with_options(m, f, ctx, &json!({"separators":""}))
}
pub(super) fn xml_with_options(
    m: &Model,
    f: &str,
    ctx: &WriterContext,
    caller: &Value,
) -> Result<Vec<Xml>, String> {
    let mut body = vec![];
    for v in list(m)? {
        if v.is_null() {
            if f == "omml" {
                return Err(source_error("Fenced Formula child has no OMML writer"));
            }
            continue;
        }
        let nodes = strict_slot_xml(&child_model(v)?, f, ctx, false)?;
        body.extend(nodes);
    }
    if f == "mathml" && ctx.intent {
        super::update_partial_arguments(&mut body);
    }
    let left = delimiter(m.field("parameter_one"), f, ctx)?;
    let right = delimiter(m.field("parameter_three"), f, ctx)?;
    if f == "mathml" {
        if !m.field("options").is_null() && !m.field("options").is_object() {
            return Err(source_error("Fence options have no dig method"));
        }
        let suppress = |s: Option<String>| {
            let s = s.unwrap_or_default();
            if s.contains(':')
                || ["&#x3016;", "&#x3017;", "&#x2524;", "&#x251c;"].contains(&s.as_str())
            {
                String::new()
            } else {
                s
            }
        };
        let mut c = vec![attrs(
            "mo",
            attributes(&m.field("options")["open_paren"])?,
            vec![xml_text(&suppress(left))],
        )];
        c.extend(body);
        c.push(attrs(
            "mo",
            attributes(&m.field("options")["close_paren"])?,
            vec![xml_text(&suppress(right))],
        ));
        return Ok(vec![el("mrow", c)]);
    }
    if f != "omml" {
        return Err(format!("unknown fence XML format {f}"));
    }
    let mut props = vec![];
    if let Some(s) = left {
        props.push(attrs(
            "m:begChr",
            vec![("m:val".into(), super::text_content::decode(&s)?)],
            vec![],
        ));
    }
    props.push(attrs(
        "m:sepChr",
        vec![("m:val".into(), scalar(&caller["separators"])?)],
        vec![],
    ));
    if let Some(s) = right {
        props.push(attrs(
            "m:endChr",
            vec![("m:val".into(), super::text_content::decode(&s)?)],
            vec![],
        ));
    }
    Ok(vec![el("m:d", vec![el("m:dPr", props), el("m:e", body)])])
}

/// Source binomial intent has precedence over interval detection; unlike the
/// Unicode choose branch it tests the flag value rather than key presence.
pub(super) fn intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    let first = m.field("parameter_two").as_array().and_then(|a| a.first());
    if let Some(v) = first {
        if choose(v) && choose_truthy(v)? {
            let children = super::xml_nodes_mut(node)?;
            let fraction = children
                .get_mut(1)
                .ok_or_else(|| source_error("Binomial intent lacks its fraction"))?;
            let fields = super::xml_nodes_mut(fraction)?;
            let numerator = if let Some(n) = fields.get_mut(0) {
                super::intent_node_value(n, "t")?
            } else {
                String::new()
            };
            let denominator = if let Some(n) = fields.get_mut(1) {
                super::intent_node_value(n, "b")?
            } else {
                String::new()
            };
            super::xml_attribute(
                node,
                "intent",
                format!("binomial-coefficient({numerator},{denominator})"),
            );
            return Ok(());
        }
    }
    super::fenced_intent(m, node)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_fenced_line_break_state() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../fixtures/fences/linebreak-reference.json"
        ))
        .unwrap();
        for row in reference["states"].as_array().unwrap() {
            let mut node = row["model"].clone();
            let mut continuation = row["pending"].as_array().unwrap().clone();
            let result = line_breaking(&mut node, &mut continuation);
            if row["expected"].get("error").is_some() {
                assert!(result.is_err(), "{}", row["id"])
            } else {
                result.unwrap();
                assert_eq!(node, row["expected"]["node"], "node {}", row["id"]);
                assert_eq!(
                    continuation,
                    row["expected"]["continuation"].as_array().unwrap().clone(),
                    "continuation {}",
                    row["id"]
                );
            }
        }
    }
    #[test]
    fn explicit_caller_options_own_omml_separators() {
        let reference: Value =
            serde_json::from_str(include_str!("../../fixtures/fences/caller-reference.json"))
                .unwrap();
        let ctx = WriterContext {table:false, formula_input:None,source_format:"asciimath", formatter: None,
            display_style: true,
            unary_function_spacing: true,
            intent: false,
        };
        for row in reference["rows"].as_array().unwrap() {
            let model = Model::from_value(&row["model"]).unwrap();
            let nodes = xml_with_options(&model, "omml", &ctx, &row["caller"]).unwrap();
            assert_eq!(
                format!("{}\n", super::super::dump(&nodes[0], 0)),
                row["output"].as_str().unwrap()
            );
        }
    }
}
