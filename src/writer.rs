//! Procedural source-writer port; unported nodes/options fail, never return approximations.
mod operators;
mod numbers;
mod multiscripts;
mod stacked;
mod accents;
mod scripts;
mod styles;
mod font_styles;
mod nary;
mod symbol_content;
mod table_xml;
mod tables;
mod templates;
mod text_content;
use crate::model::Model;
use serde_json::Value;
#[derive(Clone, Debug)]
enum Xml {
    El(String, Vec<(String, String)>, Vec<Xml>),
    Text(String),
}
fn el(n: &str, c: Vec<Xml>) -> Xml {
    Xml::El(n.into(), vec![], c)
}
fn attrs(n: &str, a: Vec<(String, String)>, c: Vec<Xml>) -> Xml {
    Xml::El(n.into(), a, c)
}
fn text(s: &str) -> Xml {
    Xml::Text(s.into())
}
fn escape(s: &str, attribute: bool) -> String {
    let filtered: String = s
        .chars()
        .filter(|c| (*c as u32) >= 0x20 || ['\t', '\n', '\r'].contains(c))
        .collect();
    let mut x = filtered.replace('<', "&lt;").replace('>', "&gt;");
    if attribute {
        x = x.replace('"', "&quot;")
    }
    x
}
fn dump(x: &Xml, depth: usize) -> String {
    match x {
        Xml::Text(s) => escape(s, false),
        Xml::El(n, a, c) => {
            let pad = "  ".repeat(depth);
            let at = a
                .iter()
                .map(|(k, v)| format!(" {k}=\"{}\"", escape(v, true)))
                .collect::<String>();
            if c.is_empty() {
                return format!("{pad}<{n}{at}/>");
            };
            if c.iter().all(|v| matches!(v, Xml::Text(_))) {
                return format!(
                    "{pad}<{n}{at}>{}</{n}>",
                    c.iter().map(|v| dump(v, 0)).collect::<String>()
                );
            };
            format!(
                "{pad}<{n}{at}>\n{}{}</{n}>",
                c.iter()
                    .filter(|v| !matches!(v, Xml::Text(s) if s.is_empty()))
                    .map(|v| dump(v, depth + 1))
                    .collect::<Vec<_>>()
                    .join("\n"),
                if matches!(c.last(), Some(Xml::Text(_))) {
                    String::new()
                } else {
                    format!("\n{pad}")
                }
            )
        }
    }
}
fn value_text(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        _ => Err("unported scalar coercion; source can distinguish string/hash/array".into()),
    }
}
fn symbol(m: &Model, f: &str) -> Result<String, String> {
    if m.kind() == "Symbol" {
        let value = value_text(m.field("value"))?;
        if f == "latex" {
            return Ok(match value.as_str() {
                "{:" | ":}" => String::new(),
                "{" | "}" | "_" => format!("\\{value}"),
                "if" => "\\operatorname{if}".into(),
                _ => value,
            });
        }
        if f == "unicodemath" {
            if m.field("mini_sub_sized") == true || m.field("mini_sup_sized") == true {
                return Err("unported promised mini-sized symbol".into());
            }
            if m.field("slashed") == true || ["&", "@", "^"].contains(&value.as_str()) {
                return Ok(format!("\\{value}"));
            }
        }
        return Ok(value);
    }
    if !m.field("value").is_null() {
        return Err("unported source symbol subclass value override".into());
    }

    let id = m
        .class
        .strip_prefix("Math::Symbols::")
        .ok_or("not a source symbol class")?;
    crate::symbols::tables()[f][id]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("unported promised symbol {}", m.class))
}
fn children(m: &Model) -> Result<Vec<Value>, String> {
    m.field("value").as_array().cloned().ok_or_else(|| {
        "formula.value must be a node array; source mixed non-node children cannot render".into()
    })
}
fn child(v: &Value, f: &str) -> Result<String, String> {
    if v.is_null() {
        Ok(String::new())
    } else {
        render_text(&Model::from_value(v)?, f)
    }
}
// Fenced#symbol_or_paren uses MathML content for HTML, and decoded
// OMML delimiter values. An ASCII spelling is not the XML delimiter.
fn fence_delimiter(value: &Value, format: &str) -> Result<String, String> {
    let model = Model::from_value(value)?;
    if model.class.starts_with("Math::Symbols::Paren::") {
        let id = model.class.strip_prefix("Math::Symbols::").unwrap();
        return match format {
            "html" | "mathml" => crate::symbols::tables()["mathml"][id]["text"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "unported source fence MathML content".into()),
            "omml" => text_content::decode(&symbol(&model, "omml")?),
            _ => child(value, format),
        };
    }
    value_text(model.field("value"))
}
fn fence_text(m: &Model, f: &str) -> Result<String, String> {
    if !m.field("parameter_one").is_object() || !m.field("parameter_three").is_object() {
        return Err("unported promised nullable/raw fence delimiter profile".into());
    }
    let options = m.field("options");
    if !options.is_null() && options.as_object().map_or(true, |o| !o.is_empty()) {
        return Err("unported promised fence options".into());
    }
    let a = if f == "html" || f == "latex" {
        fence_delimiter(m.field("parameter_one"), f)?
    } else {
        child(m.field("parameter_one"), f)?
    };
    let b = if f == "html" || f == "latex" {
        fence_delimiter(m.field("parameter_three"), f)?
    } else {
        child(m.field("parameter_three"), f)?
    };
    let body = m
        .field("parameter_two")
        .as_array()
        .ok_or("fence.parameter_two must be node array")?
        .iter()
        .map(|v| child(v, f))
        .collect::<Result<Vec<_>, _>>()?
        .join(if f == "html" { "" } else { " " });
    Ok(match f {
        "latex" => format!("{} {body} {}", if ["{:", ":}"].contains(&a.as_str()) { a.replace(':', "") } else { a }, if ["{:", ":}"].contains(&b.as_str()) { b.replace(':', "") } else { b }),
        "html" => format!("<i>{a}</i>{body}<i>{b}</i>"),
        "unicodemath" if m.field("parameter_one")["class"] == "Math::Symbols::Paren::Vert"
            || (m.field("parameter_one")["class"] == "Math::Symbols::Symbol"
                && m.field("parameter_one")["fields"]["value"]
                    .as_str().is_some_and(|s| s.contains('|'))) => format!("{a}({body}){b}"),
        _ => format!("{a}{body}{b}"),
    })
}
// Closed source classes inheriting UnaryFunction text/MathML behavior; OMML
// named-run overrides are separately declared in the corresponding source files.
pub(crate) fn unary_class(m: &Model) -> bool {
    m.class.starts_with("Math::Function::")
        && [
            "Arccos", "Arcsin", "Arctan", "Cos", "Cosh", "Cot", "Coth", "Csc", "Csch", "Sec",
            "Sech", "Sin", "Sinh", "Tan", "Tanh", "Ln", "Det", "Gcd", "Max", "Hom", "Exp", "Lcm",
            "Min", "Dim", "Glb", "Lub", "Lg", "Ker", "Deg", "Liminf", "Limsup",
        ]
        .contains(&m.kind())
}
fn present(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn unary_slot_text(v: &Value, f: &str) -> Result<String, String> {
    if !present(v) {
        return Ok(String::new());
    }
    if let Some(a) = v.as_array() {
        let sep = if f == "latex" { " " } else { "" };
        return a
            .iter()
            .filter(|v| !v.is_null())
            .map(|v| child(v, f))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join(sep));
    }
    child(v, f)
}
fn unary_text(m: &Model, f: &str) -> Result<String, String> {
    let name = m.kind().to_lowercase();
    let v = m.field("parameter_one");
    if f == "unicodemath" && !v.is_null() && !v.is_object() {
        return Err("source_render_error: Unicode unary argument must be a node; source calls to_unicodemath directly".into());
    }
    let body = unary_slot_text(v, f)?;
    match f {
        "asciimath" => {
            if m.kind() == "Lcm" {
                return Ok(if present(v) {
                    format!("lcm {body}")
                } else {
                    "lcm".into()
                });
            }
            let keywords = crate::symbols::tables()["unary_keywords"]
                .as_array()
                .unwrap();
            Ok(if keywords.iter().any(|v| v == &name) {
                format!("{name}{body}")
            } else if present(v) {
                format!("{name}({body})")
            } else {
                name
            })
        }
        "latex" => Ok(format!(
            "{}{}{{{body}}}",
            if ["Lcm", "Glb"].contains(&m.kind()) {
                ""
            } else {
                "\\"
            },
            name
        )),
        "html" => Ok(format!(
            "<i>{name}</i>{}",
            if present(v) {
                format!("<i>{body}</i>")
            } else {
                String::new()
            }
        )),
        "unicodemath" => Ok(format!("{name}⁡{body}")),
        _ => unreachable!(),
    }
}
fn unary_slot_xml(
    v: &Value,
    f: &str,
    spacing: bool,
    allow_templates: bool,
) -> Result<Vec<Xml>, String> {
    if !present(v) {
        return Ok(vec![]);
    }
    if let Some(a) = v.as_array() {
        let mut out = vec![];
        for v in a.iter().filter(|v| !v.is_null()) {
            out.extend(render_xml(
                &Model::from_value(v)?,
                f,
                spacing,
                allow_templates,
            )?)
        }
        Ok(out)
    } else {
        render_xml(&Model::from_value(v)?, f, spacing, allow_templates)
    }
}
fn run(s: &str) -> Xml {
    el("m:r", vec![el("m:t", vec![text(s)])])
}
fn word_properties(italic: bool) -> Xml {
    let mut c = vec![attrs(
        "w:rFonts",
        vec![
            ("w:ascii".into(), "Cambria Math".into()),
            ("w:hAnsi".into(), "Cambria Math".into()),
        ],
        vec![],
    )];
    if italic {
        c.push(el("w:i", vec![]))
    }
    el("w:rPr", c)
}
fn control_properties() -> Xml {
    el("m:ctrlPr", vec![word_properties(true)])
}
fn unary_xml(m: &Model, f: &str, spacing: bool, allow_templates: bool) -> Result<Vec<Xml>, String> {
    let name = m.kind().to_lowercase();
    let param = m.field("parameter_one");
    let hide = present(m.field("hide_function_name"));
    let body = unary_slot_xml(param, f, spacing, allow_templates)?;
    if f == "mathml" {
        let mi = crate::symbols::tables()["unary_mi"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == &name);
        let mut c = vec![];
        if !hide {
            c.push(el(if mi { "mi" } else { "mo" }, vec![text(&name)]))
        }
        let value = if present(param) {
            c.extend(body);
            vec![el("mrow", c)]
        } else {
            c
        };
        if mi && spacing {
            let mut children = vec![attrs(
                "mo",
                vec![("rspace".into(), "thickmathspace".into())],
                vec![],
            )];
            children.extend(value);
            return Ok(vec![el("mrow", children)]);
        }
        return Ok(value);
    }
    let named = [
        "Ln", "Det", "Gcd", "Max", "Hom", "Exp", "Lcm", "Min", "Dim", "Glb", "Lub", "Lg", "Ker",
        "Deg", "Liminf", "Limsup",
    ]
    .contains(&m.kind());
    if named {
        let mut c = vec![];
        if !hide {
            c.push(run(&name))
        }
        c.extend(body);
        return Ok(c);
    }
    if !present(param) {
        return Ok(vec![run(&name)]);
    }
    if hide {
        return Ok(body);
    }
    Ok(vec![el(
        "m:func",
        vec![
            el("m:funcPr", vec![control_properties()]),
            el(
                "m:fName",
                vec![el(
                    "m:r",
                    vec![word_properties(false), el("m:t", vec![text(&name)])],
                )],
            ),
            el("m:e", body),
        ],
    )])
}
fn parens(v: &Value, f: &str) -> Result<String, String> {
    if v.is_null() || !v.is_object() {
        return Err("source_render_error: argument must receive its source node writer".into());
    }
    let out = child(v, f)?;
    Ok(if v["class"] == "Math::Function::Fenced" {
        out
    } else {
        format!("({out})")
    })
}
fn special_text(m: &Model, f: &str) -> Result<String, String> {
    let v = m.field("parameter_one");
    match(m.kind(),f){
 ("Phantom","asciimath")=>Ok("\\ ".repeat(unary_slot_text(v,f)?.chars().count())),
 ("Phantom","html")=>Err("source_render_error: Phantom#to_html accepts no options keyword at the formula writer boundary".into()),
 ("Phantom","unicodemath")=>{if v["class"]=="Math::Function::Mpadded"{return Err("unported promised conditional phantom/mpadded profile".into())}Ok(format!("⟡{}",parens(v,f)?))},
 ("Cancel","unicodemath")=>Ok(format!("╱{}",if present(v){parens(v,f)?}else{String::new()})),
 _=>unary_text(m,f)}
}
fn strict_slot_xml(
    v: &Value,
    f: &str,
    spacing: bool,
    allow_templates: bool,
    allow_array: bool,
) -> Result<Vec<Xml>, String> {
    if v.is_null() {
        return Ok(vec![]);
    }
    if allow_array && v.is_array() {
        let mut c = vec![];
        for v in v.as_array().unwrap().iter().filter(|v| !v.is_null()) {
            if !v.is_object() {
                return Err("source_render_error: XML argument is not a node".into());
            }
            c.extend(render_xml(
                &Model::from_value(v)?,
                f,
                spacing,
                allow_templates,
            )?)
        }
        return Ok(c);
    }
    if !v.is_object() {
        return Err("source_render_error: XML argument is not a node".into());
    }
    render_xml(&Model::from_value(v)?, f, spacing, allow_templates)
}
fn special_xml(
    m: &Model,
    f: &str,
    spacing: bool,
    allow_templates: bool,
) -> Result<Vec<Xml>, String> {
    let v = m.field("parameter_one");
    let body = strict_slot_xml(
        v,
        f,
        spacing,
        allow_templates,
        m.kind() == "Phantom" || f == "omml",
    )?;
    if m.kind() == "Phantom" {
        return Ok(vec![if f == "mathml" {
            el("mphantom", body)
        } else {
            el(
                "m:phant",
                vec![
                    el(
                        "m:phantPr",
                        vec![attrs(
                            "m:show",
                            vec![("m:val".into(), "off".into())],
                            vec![],
                        )],
                    ),
                    el("m:e", body),
                ],
            )
        }]);
    }
    if f == "mathml" {
        return Ok(vec![attrs(
            "menclose",
            vec![("notation".into(), "updiagonalstrike".into())],
            body,
        )]);
    }
    if body.is_empty() {
        return Ok(vec![]);
    }
    let props = ["hideTop", "hideBot", "hideLeft", "hideRight", "strikeBLTR"]
        .iter()
        .map(|name| {
            attrs(
                &format!("m:{name}"),
                vec![("m:val".into(), "on".into())],
                vec![],
            )
        })
        .collect();
    Ok(vec![el(
        "m:borderBox",
        vec![el("m:borderBoxPr", props), el("m:e", body)],
    )])
}
fn prime_symbol(v: &Value) -> Result<bool, String> {
    let Ok(m) = Model::from_value(v) else {
        return Ok(false);
    };
    if !m.class.starts_with("Math::Symbols::") {
        return Ok(false);
    }
    let txt = render_text(&m, "unicodemath")?;
    Ok([
        "′", "″", "‴", "⁗", "&#x27;", "&#x2032;", "&#x2033;", "&#x2034;", "&#x2057;",
    ]
    .iter()
    .any(|p| txt.contains(p)))
}
fn power_accent(v: &Value) -> Result<bool, String> {
    let Ok(m) = Model::from_value(v) else {
        return Ok(false);
    };
    if m.kind() == "Formula" {
        return prime_symbol(&m.field("value")[0]);
    }
    if m.kind() == "Power" {
        return prime_symbol(m.field("parameter_one"));
    }
    prime_symbol(v)
}
fn power_text(m: &Model, f: &str) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    match f {
        "asciimath" => {
            if a.is_null() {
                return Err("source_render_error: power base cannot receive to_asciimath".into());
            }
            let base = child(a, f)?;
            Ok(if present(b) {
                format!("{base}^({})", child(b, f)?)
            } else {
                base
            })
        }
        "latex" => {
            if a.is_null() {
                return Err("source_render_error: power base cannot receive to_latex".into());
            }
            Ok(format!(
                "{}^{{{}}}",
                child(a, f)?,
                if present(b) {
                    child(b, f)?
                } else {
                    String::new()
                }
            ))
        }
        "html" => Ok(format!(
            "{}{}",
            if present(a) {
                let value = child(a, f)?;
                if a["class"] == "Math::Function::FontStyle::Normal" {
                    value
                } else {
                    format!("<i>{value}</i>")
                }
            } else {
                String::new()
            },
            if present(b) {
                format!("<sup>{}</sup>", child(b, f)?)
            } else {
                String::new()
            }
        )),
        "unicodemath" => {
            if b.is_null() || !b.is_object() {
                return Err(
                    "source_render_error: power exponent cannot receive mini_sized?".into(),
                );
            }
            if b["fields"]["mini_sub_sized"] == true || b["fields"]["mini_sup_sized"] == true {
                return Err("unported promised mini-sized power exponent".into());
            }
            let base = child(a, f)?;
            if power_accent(b)? {
                return Ok(format!(
                    "{base}{}",
                    child(b, f)?
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace())
                        .collect::<String>()
                ));
            }
            Ok(format!(
                "{base}^{}",
                if b["class"] == "Math::Function::Power" {
                    child(b, f)?
                } else {
                    parens(b, f)?
                }
            ))
        }
        _ => unreachable!(),
    }
}
fn power_xml(m: &Model, f: &str, spacing: bool, allow_templates: bool) -> Result<Vec<Xml>, String> {
    let a = if f == "omml" && !present(m.field("parameter_one")) {
        vec![run("&#8203;")]
    } else {
        strict_slot_xml(m.field("parameter_one"), f, spacing, allow_templates, true)?
    };
    let b = if f == "omml" && !present(m.field("parameter_two")) {
        vec![run("&#8203;")]
    } else {
        strict_slot_xml(m.field("parameter_two"), f, spacing, allow_templates, true)?
    };
    if f == "mathml" {
        let mut c = a;
        c.extend(b);
        let tag = if ["Math::Function::Ubrace", "Math::Function::Obrace"]
            .iter()
            .any(|k| m.field("parameter_one")["class"] == *k)
        {
            "mover"
        } else {
            "msup"
        };
        return Ok(vec![el(tag, c)]);
    }
    Ok(vec![el(
        "m:sSup",
        vec![
            el("m:sSupPr", vec![control_properties()]),
            el("m:e", a),
            el("m:sup", b),
        ],
    )])
}
fn derived_units(m: &Model) -> Result<Model, String> {
    let value = m
        .field("text")
        .as_str()
        .ok_or("Unitsml.text must be a string")?;
    let formula = crate::units_parse::formula(value, &serde_json::json!({}))
        .map_err(|e| format!("unported UnitsML derived-formula profile: {e}"))?;
    Model::from_value(&formula)
}
fn unit_unicode_context(v: &mut Value) {
    if !v.is_object() {
        return;
    }
    if v["class"] == "Math::Function::FontStyle::Normal"
        && v["fields"]["parameter_one"]["class"] == "Math::Symbols::Symbol"
    {
        if let Some(value) = v["fields"]["parameter_one"]["fields"]["value"].as_str() {
            if !value.starts_with('\\')
                && text_content::decode(value).map_or(false, |s| s.chars().count() > 1)
            {
                let text = value.to_owned();
                v["fields"]["parameter_one"] = crate::model::node(
                    "Math::Function::Text",
                    serde_json::json!({"parameter_one":text,"lang":null}),
                );
            }
        }
    }
    if let Some(fields) = v.get_mut("fields").and_then(Value::as_object_mut) {
        for x in fields.values_mut() {
            if x.is_object() {
                unit_unicode_context(x)
            } else if let Some(a) = x.as_array_mut() {
                for x in a {
                    unit_unicode_context(x)
                }
            }
        }
    }
}
fn normal_text(m: &Model, f: &str) -> Result<String, String> {
    let p = m.field("parameter_one");
    if !p.is_null() && !p.is_object() {
        return Err(
            "source_render_error: Normal argument cannot receive its source node writer".into(),
        );
    }
    let s = child(p, f)?;
    Ok(match f {
        "asciimath" => format!("rm({s})"),
        "latex" => format!("\\mathrm{{{s}}}"),
        "html" => s,
        "unicodemath" => {
            let token = m.field("parameter_two").as_str().unwrap_or("rm");
            if crate::symbols::tables()["font_map"][token] != "Math::Function::FontStyle::Normal" {
                return Err("unported source Normal font-family token".into());
            }
            format!("\\mup{s}")
        }
        _ => unreachable!(),
    })
}
fn upright_properties() -> Xml {
    el(
        "m:rPr",
        vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
    )
}
fn upright(node: &mut Xml) {
    if let Xml::El(name, _, children) = node {
        if name == "m:r" {
            if !children
                .iter()
                .any(|c| matches!(c,Xml::El(n,_,_)if n=="m:rPr"))
            {
                children.insert(0, upright_properties())
            }
        } else {
            for c in children {
                upright(c)
            }
        }
    }
}
fn normal_xml(
    m: &Model,
    f: &str,
    spacing: bool,
    allow_templates: bool,
) -> Result<Vec<Xml>, String> {
    let p = m.field("parameter_one");
    if p.is_null() {
        return Ok(vec![if f == "mathml" {
            attrs(
                "mstyle",
                vec![("mathvariant".into(), "normal".into())],
                vec![],
            )
        } else {
            el("m:r", vec![upright_properties()])
        }]);
    }
    if !p.is_object() {
        return Err(
            "source_render_error: Normal argument cannot receive its source XML writer".into(),
        );
    }
    let child = Model::from_value(p)?;
    let mut c = if f == "omml" && child.kind() == "Text" {
        text_content::direct_omml(&child)?
    } else {
        render_xml(&child, f, spacing, allow_templates)?
    };
    if f == "mathml" {
        return Ok(vec![attrs(
            "mstyle",
            vec![("mathvariant".into(), "normal".into())],
            c,
        )]);
    }
    if child.kind() == "Number" || child.class.starts_with("Math::Symbols::") {
        c = c
            .into_iter()
            .flat_map(|x| match x {
                Xml::El(n, _, children) if n == "m:r" => children,
                _ => vec![x],
            })
            .collect()
    }
    if c.iter()
        .all(|x| matches!(x, Xml::Text(_)) || matches!(x,Xml::El(n,_,_)if n=="m:t"))
    {
        let mut children = vec![upright_properties()];
        children.extend(c);
        return Ok(vec![el("m:r", children)]);
    }
    for x in &mut c {
        upright(x)
    }
    Ok(c)
}
fn render_text(m: &Model, f: &str) -> Result<String, String> {
    if numbers::handles(m) { return numbers::text(m, f); }
    if ["Math::Function::Multiscript", "Math::Function::Linebreak"].contains(&m.class.as_str()) { return multiscripts::text(m, f); }
    if stacked::handles_text(m) { return stacked::text(m, f); }
    if m.class == "Math::Function::PowerBase" { return operators::text_writer(m,f); }
    if accents::handles(m) {
        return accents::text(m, f);
    }
    if m.kind() == "Color" {
        return styles::text(m, f);
    }
    if m.kind() == "Base" {
        return scripts::text(m, f);
    }
    if m.class.starts_with("Math::Function::Table::") {
        return tables::render(m, f);
    }
    if m.kind() == "Unitsml" {
        let model = derived_units(m)?;
        if f == "unicodemath" {
            let mut v = model.value();
            unit_unicode_context(&mut v);
            return render_text(&Model::from_value(&v)?, f);
        }
        return render_text(&model, f);
    }
    if m.class == "Math::Function::FontStyle::Normal" {
        return normal_text(m, f);
    }
    if font_styles::handles(m) { return font_styles::text(m, f); }
    if nary::handles(m) { return nary::text(m, f); }
    if ["Text", "Mbox", "Left", "Right"].contains(&m.kind()) {
        return text_content::text_writer(m, f);
    }
    if m.kind() == "Power" {
        return power_text(m, f);
    }
    if ["Phantom", "Cancel"].contains(&m.kind()) {
        return special_text(m, f);
    }
    if unary_class(m) {
        return unary_text(m, f);
    }
    if m.class.starts_with("Math::Symbols::") {
        return symbol(m, f);
    }
    match m.kind() {
        "Fenced" => fence_text(m, f),
        "Formula" | "Mrow" | "Mstyle" => {
            let c = children(m)?
                .iter()
                .map(|v| child(v, f))
                .collect::<Result<Vec<_>, _>>()?;
            let joined = c.join(if f == "unicodemath" && m.kind() == "Formula" { numbers::unicode_separator(m) } else { " " });
            if f == "unicodemath" {
                static SLASH_SPACING: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                let spacing = SLASH_SPACING.get_or_init(|| regex::Regex::new(r"(?-u:\s)/(?-u:\s)").unwrap());
                text_content::decode(&joined).map(|s| spacing.replace_all(&s, "/").into_owned())
            } else {
                Ok(joined)
            }
        }
        "Number" => {
            if !m.field("base").is_null()
                || m.field("mini_sub_sized") == true
                || m.field("mini_sup_sized") == true
            {
                return Err("number base/mini formatting not yet ported".into());
            }
            value_text(m.field("value"))
        }
        "Symbol" | "Plus" | "Minus" | "Equal" | "Asterisk" | "Times" => symbol(m, f),
        "Frac" => {
            if !m.field("options").is_null() {
                return Err("fraction option profile not yet ported".into());
            }
            let a = child(m.field("parameter_one"), f)?;
            let b = child(m.field("parameter_two"), f)?;
            Ok(match f {
                "asciimath" => format!("frac{}{}",
                    if present(m.field("parameter_one")) {format!("({a})")} else {String::new()},
                    if present(m.field("parameter_two")) {format!("({b})")} else {String::new()}),
                "latex" => format!("\\frac{{{a}}}{{{b}}}"),
                "html" => format!(
                    "{}{}",
                    if m.field("parameter_one").is_null() {
                        String::new()
                    } else {
                        format!("<i>{a}</i>")
                    },
                    if m.field("parameter_two").is_null() {
                        String::new()
                    } else {
                        format!("<i>{b}</i>")
                    }
                ),
                "unicodemath" => format!(
                    "{}/{}",
                    if m.field("parameter_one").is_null() {
                        String::new()
                    } else if m.field("parameter_one")["class"] == "Math::Function::Fenced" {
                        a
                    } else {
                        format!("({a})")
                    },
                    if m.field("parameter_two").is_null() {
                        String::new()
                    } else if m.field("parameter_two")["class"] == "Math::Function::Fenced" {
                        b
                    } else {
                        format!("({b})")
                    }
                ),
                _ => unreachable!(),
            })
        }
        _ => templates::text_template(m, f)
            .unwrap_or_else(|| Err(format!("unported promised writer node {}", m.class))),
    }
}
fn render_xml(
    m: &Model,
    f: &str,
    spacing: bool,
    allow_templates: bool,
) -> Result<Vec<Xml>, String> {
    if numbers::handles(m) { return numbers::xml(m, f); }
    if ["Math::Function::Multiscript", "Math::Function::Linebreak"].contains(&m.class.as_str()) { return multiscripts::xml(m, f, spacing, allow_templates); }
    if ["Substack", "Msline", "Mglyph"].contains(&m.kind()) { return stacked::xml(m, f, spacing, allow_templates); }
    if m.class=="Math::Function::PowerBase" || (f=="omml" && ["Math::Function::Mod","Math::Function::Sqrt"].contains(&m.class.as_str())) { return operators::xml_writer(m,f,spacing,allow_templates); }

    if accents::handles(m) {
        return accents::xml(m, f, spacing, allow_templates);
    }
    if m.kind() == "Color" {
        return styles::xml(m, f, spacing, allow_templates);
    }
    if m.kind() == "Base" {
        return scripts::xml(m, f, spacing, allow_templates);
    }
    if matches!(
        m.class.as_str(),
        "Math::Function::Table"
            | "Math::Function::Table::Matrix"
            | "Math::Function::Table::Pmatrix"
            | "Math::Function::Table::Bmatrix"
            | "Math::Function::Table::Vmatrix"
            | "Math::Function::Table::Array"
            | "Math::Function::Tr"
            | "Math::Function::Td"
    ) {
        return table_xml::render(m, f, spacing, allow_templates);
    }

    if m.kind() == "Unitsml" {
        return render_xml(&derived_units(m)?, f, spacing, allow_templates);
    }
    if m.class == "Math::Function::FontStyle::Normal" {
        return normal_xml(m, f, spacing, allow_templates);
    }
    if font_styles::handles(m) { return font_styles::xml(m, f, spacing, allow_templates); }
    if nary::handles(m) { return nary::xml(m, f, spacing, allow_templates); }

    if ["Text", "Mbox", "Left", "Right"].contains(&m.kind()) {
        return text_content::xml_writer(m, f);
    }
    if m.kind() == "Power" {
        return power_xml(m, f, spacing, allow_templates);
    }
    if ["Phantom", "Cancel"].contains(&m.kind()) {
        return special_xml(m, f, spacing, allow_templates);
    }
    if unary_class(m) {
        return unary_xml(m, f, spacing, allow_templates);
    }
    if m.class.starts_with("Math::Symbols::") {
        if m.class == "Math::Symbols::Symbol" {
            return symbol_content::xml_writer(m, f);
        }
        let (tag, s) = if m.kind() == "Symbol" {
            ("mi".to_owned(), value_text(m.field("value"))?)
        } else {
            let id = m.class.strip_prefix("Math::Symbols::").unwrap();
            if f == "mathml" {
                let d = &crate::symbols::tables()["mathml"][id];
                (
                    d["tag"]
                        .as_str()
                        .ok_or("missing source MathML symbol descriptor")?
                        .to_owned(),
                    d["text"].as_str().unwrap().to_owned(),
                )
            } else {
                ("".into(), symbol(m, f)?)
            }
        };
        return Ok(vec![if f == "mathml" {
            el(&tag, vec![text(&s)])
        } else {
            el("m:r", vec![el("m:t", vec![text(&s)])])
        }]);
    }
    match m.kind() {
        "Fenced" => {
            if !m.field("parameter_one").is_object() || !m.field("parameter_three").is_object() {
                return Err("unported promised nullable/raw fence delimiter profile".into());
            }
            let a = fence_delimiter(m.field("parameter_one"), f)?;
            let b = fence_delimiter(m.field("parameter_three"), f)?;
            let options = m.field("options");
            if !options.is_null() && options.as_object().map_or(true, |o| !o.is_empty()) {
                return Err("unported promised fence options".into());
            }
            let mut body = vec![];
            for v in m
                .field("parameter_two")
                .as_array()
                .ok_or("fence.parameter_two must be a list")?
            {
                body.extend(render_xml(
                    &Model::from_value(v)?,
                    f,
                    spacing,
                    allow_templates,
                )?)
            }
            if f == "mathml" {
                let mut c = vec![el("mo", vec![text(&a)])];
                c.extend(body);
                c.push(el("mo", vec![text(&b)]));
                Ok(vec![el("mrow", c)])
            } else {
                let mut props = vec![];
                let has_delimiter = |v: &Value| v["class"].as_str().is_some_and(|c| c.starts_with("Math::Symbols::Paren::")) || !v["fields"]["value"].is_null();
                if has_delimiter(m.field("parameter_one")) {
                    props.push(attrs("m:begChr", vec![("m:val".into(), a)], vec![]));
                }
                props.push(attrs("m:sepChr", vec![("m:val".into(), "".into())], vec![]));
                if has_delimiter(m.field("parameter_three")) {
                    props.push(attrs("m:endChr", vec![("m:val".into(), b)], vec![]));
                }
                let props = el("m:dPr", props);
                Ok(vec![el("m:d", vec![props, el("m:e", body)])])
            }
        }
        "Formula" | "Mrow" | "Mstyle" => {
            let mut c = vec![];
            for v in children(m)? {
                c.extend(render_xml(
                    &Model::from_value(&v)?,
                    f,
                    spacing,
                    allow_templates,
                )?)
            }
            if f == "mathml"
                && m.field("left_right_wrapper") != false
                && !m.field("left_right_wrapper").is_null()
            {
                Ok(vec![el("mrow", c)])
            } else {
                Ok(c)
            }
        }
        "Number" => {
            let s = render_text(m, "asciimath")?;
            Ok(vec![if f == "mathml" {
                el("mn", vec![text(&s)])
            } else {
                el("m:r", vec![el("m:t", vec![text(&s)])])
            }])
        }
        "Symbol" | "Plus" | "Minus" | "Equal" | "Asterisk" | "Times" => {
            let s = symbol(m, f)?;
            Ok(vec![if f == "mathml" {
                el(
                    if m.kind() == "Symbol" { "mi" } else { "mo" },
                    vec![text(&s)],
                )
            } else {
                el("m:r", vec![el("m:t", vec![text(&s)])])
            }])
        }
        "Frac" => {
            if !m.field("options").is_null() {
                return Err("fraction option profile not yet ported".into());
            }
            let slot = |key: &str| -> Result<Vec<Xml>, String> {
                let v = m.field(key);
                if f == "omml" && !present(v) {
                    Ok(vec![run("&#8203;")])
                } else if v.is_null() {
                    Ok(vec![])
                } else {
                    render_xml(&Model::from_value(v)?, f, spacing, allow_templates)
                }
            };
            let a = slot("parameter_one")?;
            let b = slot("parameter_two")?;
            if f == "mathml" {
                let mut c = a;
                c.extend(b);
                Ok(vec![el(
                    if m.field("hide_function_name") == true {
                        "mrow"
                    } else {
                        "mfrac"
                    },
                    c,
                )])
            } else {
                Ok(vec![el(
                    "m:f",
                    vec![
                        el(
                            "m:fPr",
                            vec![el(
                                "m:ctrlPr",
                                vec![el(
                                    "w:rPr",
                                    vec![
                                        attrs(
                                            "w:rFonts",
                                            vec![
                                                ("w:ascii".into(), "Cambria Math".into()),
                                                ("w:hAnsi".into(), "Cambria Math".into()),
                                            ],
                                            vec![],
                                        ),
                                        el("w:i", vec![]),
                                    ],
                                )],
                            )],
                        ),
                        el("m:num", a),
                        el("m:den", b),
                    ],
                )])
            }
        }
        _ => templates::xml_template(m, f, spacing, allow_templates)
            .unwrap_or_else(|| Err(format!("unported promised writer node {}", m.class))),
    }
}
pub fn write(model: &Value, format: &str, options: &Value) -> Result<String, String> {
    let m = Model::from_value(model)?;
    let opts = options.as_object().ok_or("options must be an object")?;
    for (k, v) in opts {
        if ![
            "displayStyle",
            "intent",
            "splitOnLinebreak",
            "unaryFunctionSpacing",
        ]
        .contains(&k.as_str())
        {
            return Err(format!("unported promised writer option {k}"));
        }
        if !v.is_null() && k != "displayStyle" {
            if ((k == "intent" || k == "splitOnLinebreak") && v == false)
                || (k == "unaryFunctionSpacing" && v.is_boolean())
            {
            } else {
                return Err(format!("unported promised writer option {k}={v}"));
            }
        }
    }
    if ![
        "asciimath",
        "latex",
        "html",
        "unicodemath",
        "mathml",
        "omml",
    ]
    .contains(&format)
    {
        return Err(format!("unknown writer format {format}"));
    }
    if !["mathml", "omml"].contains(&format) {
        if !opts.is_empty() {
            return Err("writer options not yet ported for text formats".into());
        }
        return render_text(&m, format);
    }
    let spacing = opts
        .get("unaryFunctionSpacing")
        .map(present)
        .unwrap_or(true);
    let ds = opts
        .get("displayStyle")
        .unwrap_or_else(|| m.field("displaystyle"));
    let allow_templates =
        opts.is_empty() && spacing && (format == "mathml" || ds == true || ds == "true");
    let c = if m.kind() == "Formula" {
        let mut c = vec![];
        for v in children(&m)? {
            c.extend(render_xml(
                &Model::from_value(&v)?,
                format,
                spacing,
                allow_templates,
            )?)
        }
        c
    } else {
        render_xml(&m, format, spacing, allow_templates)?
    };
    if format == "mathml" {
        let ds = opts
            .get("displayStyle")
            .unwrap_or_else(|| m.field("displaystyle"));
        let ds = if ds == true || ds == "true" {
            "true"
        } else {
            "false"
        };
        Ok(format!(
            "{}\n",
            dump(
                &attrs(
                    "math",
                    vec![
                        ("xmlns".into(), "http://www.w3.org/1998/Math/MathML".into()),
                        ("display".into(), "block".into())
                    ],
                    vec![attrs("mstyle", vec![("displaystyle".into(), ds.into())], c)]
                ),
                0
            )
        ))
    } else {
        let ns: Vec<(String, String)> =
            serde_json::from_str(include_str!("omml-namespaces.json")).unwrap();
        Ok(format!(
            "{}\n",
            dump(&attrs("m:oMathPara", ns, vec![el("m:oMath", c)]), 0)
        ))
    }
}
