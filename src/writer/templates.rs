//! Source-produced parameterized writer skeletons. No complete-input answer table.
//! Admission is class/field/shape/default-context guarded; recursive slot output
//! comes from the native source writers. Unmatched profiles remain incomplete.
use super::WriterContext;
use super::{attrs, el, render_text, render_xml, symbol, text, Model, Xml};
use serde_json::Value;
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("templates.json"))
            .expect("sealed source writer templates")
    })
}
fn guard(v: &Value, g: &Value) -> bool {
    let shape = g["shape"].as_str().unwrap_or("");
    if shape == "null" {
        return v.is_null();
    }
    if shape == "primitive_string" {
        return v.is_string();
    }
    if shape == "array" {
        let Some(a) = v.as_array() else { return false };
        return a.len() == g["length"].as_u64().unwrap_or(0) as usize
            && a.len() == 1
            && guard(
                &a[0],
                &serde_json::json!({"shape":"symbol","class_name":g["element_class"],"value_type":"string"}),
            );
    }
    let Ok(m) = Model::from_value(v) else {
        return false;
    };
    if m.class != g["class_name"].as_str().unwrap_or("") {
        return false;
    }
    let allowed: &[&str] = match shape {
        "formula" => &["displaystyle", "left_right_wrapper", "value"],
        "number" => &["base", "mini_sub_sized", "mini_sup_sized", "value"],
        "symbol" => &["value"],
        _ => return false,
    };
    if m.fields.keys().any(|k| !allowed.contains(&k.as_str())) {
        return false;
    }
    if let Some(eq) = g["field_equals"].as_object() {
        if eq.iter().any(|(k, e)| m.field(k) != e) {
            return false;
        }
    }
    if g["value_type"] == "string" && !m.field("value").is_string() {
        return false;
    }
    if g["nonempty_value"] == true && m.field("value").as_array().map_or(true, |a| a.is_empty()) {
        return false;
    }
    true
}
fn eligible(p: &Value, m: &Model, f: &str) -> bool {
    if p["class"] != m.class || p["format"] != f {
        return false;
    }
    if p["profile_options"]
        .as_object()
        .map_or(true, |a| !a.is_empty())
    {
        return false;
    }
    let Some(allowed) = p["allowed_fields"].as_array() else {
        return false;
    };
    if m.fields.keys().any(|k| !allowed.iter().any(|a| a == k)) {
        return false;
    }
    if let Some(eq) = p["scalar_defaults"].as_object() {
        if eq.iter().any(|(k, e)| m.field(k) != e) {
            return false;
        }
    }
    let Some(gs) = p["slot_guards"].as_object() else {
        return false;
    };
    gs.iter().all(|(k, g)| guard(m.field(k), g))
}
fn find<'a>(m: &Model, f: &str, kind: &str) -> Option<&'a Value> {
    data()["profiles"]
        .as_array()?
        .iter()
        .find(|p| p["template"]["kind"] == kind && eligible(p, m, f))
}
fn slot<'a>(p: &'a Value, name: &str) -> Result<&'a Value, String> {
    p["slots"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["field"] == name))
        .ok_or_else(|| format!("source template has no slot {name}"))
}
fn value<'a>(m: &'a Model, s: &Value) -> Result<&'a Value, String> {
    let a = s["path"]
        .as_array()
        .ok_or("source template slot path missing")?;
    let Some(first) = a.first().and_then(Value::as_str) else {
        return Err("source template slot root missing".into());
    };
    let mut v = m.field(first);
    for key in a.iter().skip(1) {
        v = if let Some(i) = key.as_u64() {
            v.get(i as usize)
        } else {
            key.as_str().and_then(|k| v.get(k))
        }
        .ok_or("source template slot path does not exist")?
    }
    Ok(v)
}
fn slot_text(m: &Model, p: &Value, name: &str, f: &str) -> Result<String, String> {
    let s = slot(p, name)?;
    let v = value(m, s)?;
    if s.get("call").is_none() {
        return v
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "source primitive slot is not a string".into());
    }
    let method = s["call"]["method"].as_str().unwrap_or("");
    if method != format!("to_{f}") {
        return Err(format!("unported source template text call {method}"));
    }
    render_text(&Model::from_value(v)?, f)
}
fn parts(m: &Model, p: &Value, ps: &Value, f: &str) -> Result<String, String> {
    let mut out = String::new();
    for part in ps.as_array().ok_or("source template parts not an array")? {
        if let Some(l) = part["literal"].as_str() {
            out.push_str(l)
        } else if let Some(name) = part["slot"].as_str() {
            out.push_str(&slot_text(m, p, name, f)?)
        } else {
            return Err("unrecognized source template text part".into());
        }
    }
    Ok(out)
}
pub(super) fn text_template(m: &Model, f: &str) -> Option<Result<String, String>> {
    let p = find(m, f, "text")?;
    Some(parts(m, p, &p["template"]["parts"], f))
}
fn slot_xml(
    m: &Model,
    p: &Value,
    name: &str,
    f: &str,
    ctx: &WriterContext,
) -> Result<Vec<Xml>, String> {
    let s = slot(p, name)?;
    let v = value(m, s)?;
    let child = Model::from_value(v)?;
    let method = s["call"]["method"].as_str().unwrap_or("");
    match (f, method) {
        ("mathml", "to_mathml_without_math_tag") => render_xml(&child, f, ctx),
        ("omml", "to_omml_without_math_tag") => {
            if child.kind() == "Number" {
                let val = render_text(&child, "asciimath")?;
                return Ok(vec![el("m:t", vec![text(&val)])]);
            }
            if child.class.starts_with("Math::Symbols::") {
                let val = symbol(&child, "omml")?;
                return Ok(vec![text(&val)]);
            }
            render_xml(&child, f, ctx)
        }
        _ => Err(format!("unported source template XML slot call {method}")),
    }
}
fn nodes(
    m: &Model,
    p: &Value,
    ns: &Value,
    f: &str,
    ctx: &WriterContext,
) -> Result<Vec<Xml>, String> {
    let mut out = vec![];
    for n in ns.as_array().ok_or("source template nodes not an array")? {
        match n["kind"].as_str() {
            Some("slot") => out.extend(slot_xml(
                m,
                p,
                n["slot"].as_str().ok_or("source slot name missing")?,
                f,
                ctx,
            )?),
            Some("text") => out.push(text(&parts(m, p, &n["parts"], f)?)),
            Some("element") => {
                let name = n["name"].as_str().ok_or("source element name missing")?;
                let mut a = vec![];
                for attr in n["attributes"]
                    .as_array()
                    .ok_or("source element attributes missing")?
                {
                    a.push((
                        attr[0]
                            .as_str()
                            .ok_or("source attribute name missing")?
                            .to_owned(),
                        parts(m, p, &attr[1], f)?,
                    ))
                }
                out.push(attrs(name, a, nodes(m, p, &n["children"], f, ctx)?))
            }
            _ => return Err("unrecognized source template XML node".into()),
        }
    }
    Ok(out)
}
pub(super) fn xml_template(
    m: &Model,
    f: &str,
    ctx: &WriterContext,
) -> Option<Result<Vec<Xml>, String>> {
    let p = find(m, f, "xml")?;
    Some(nodes(m, p, &p["template"]["nodes"], f, ctx))
}
