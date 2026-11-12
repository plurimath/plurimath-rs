//! Procedural source-writer port; unported nodes/options fail, never return approximations.
mod uncertainty_writers;
mod layout_nodes;
mod generic_nary;
mod final_options;
mod mod_log;
mod portable_functions;
mod intent_nodes;
mod leaf_nodes;
mod operators;
mod numbers;
mod number_formatter;
mod formatter_context;
mod multiscripts;
mod stacked;
mod accents;
mod scripts;
mod styles;
mod font_styles;
mod fences;
mod line_break_overrides;
mod nary;
mod symbol_content;
mod table_xml;
mod tables;
mod table_text;
mod underline;
mod templates;
mod text_content;
use crate::model::Model;
use serde_json::Value;
#[derive(Clone, Debug)]
enum Xml {
    El(String, Vec<(String, String)>, Vec<Xml>),
    Text(String),
}
/// Per-call source writer state, copied only when a source method overrides it.
/// Template admission is separate from display style and child options.
#[derive(Clone, Copy)]
struct WriterContext<'a> {
    formatter: Option<&'a formatter_context::FormatterRequest>,
    formula_input: Option<&'a Value>,
    table: bool,
    source_format: &'a str,
    display_style: bool,
    unary_function_spacing: bool,
    intent: bool,
}
impl<'a> WriterContext<'a> {
    fn with_table(self)->Self {Self {table:true,..self}}
    fn with_formula<'b>(&'b self, model:&'b Model)->WriterContext<'b> {
        WriterContext {table:self.table,formatter:self.formatter,formula_input:Some(model.field("input_string")),source_format:self.source_format,display_style:self.display_style,unary_function_spacing:self.unary_function_spacing,intent:self.intent}
    }
    fn with_display_style(self, display_style: bool) -> Self {
        Self {
            display_style,
            ..self
        }
    }
}
impl Default for WriterContext<'_> {
 fn default()->Self {Self {display_style:true,unary_function_spacing:true,intent:false,formatter:None,table:false,formula_input:None,source_format:"asciimath"}}
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
            if let Some(value)=fences::symbol_unicode(&m.value())? {return Ok(value);}
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
fn child(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if v.is_null() {
        Ok(String::new())
    } else {
        render_text(&Model::from_value(v)?, f, ctx)
    }
}
// Fenced#symbol_or_paren uses MathML content for HTML, and decoded
// OMML delimiter values. An ASCII spelling is not the XML delimiter.
fn fence_delimiter(value: &Value, format: &str, ctx: &WriterContext) -> Result<String, String> {
    let model = Model::from_value(value)?;
    if model.class.starts_with("Math::Symbols::Paren::") {
        let id = model.class.strip_prefix("Math::Symbols::").unwrap();
        return match format {
            "html" | "mathml" => crate::symbols::tables()["mathml"][id]["text"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "unported source fence MathML content".into()),
            "omml" => text_content::decode(&symbol(&model, "omml")?),
            _ => child(value, format, ctx),
        };
    }
    value_text(model.field("value"))
}
fn fence_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if !m.field("parameter_one").is_object() || !m.field("parameter_three").is_object() {
        return Err("unported promised nullable/raw fence delimiter profile".into());
    }
    let options = m.field("options");
    if !options.is_null() && options.as_object().is_none_or(|o| !o.is_empty()) {
        return Err("unported promised fence options".into());
    }
    let a = if f == "html" || f == "latex" {
        fence_delimiter(m.field("parameter_one"), f, ctx)?
    } else {
        child(m.field("parameter_one"), f, ctx)?
    };
    let b = if f == "html" || f == "latex" {
        fence_delimiter(m.field("parameter_three"), f, ctx)?
    } else {
        child(m.field("parameter_three"), f, ctx)?
    };
    let body = m
        .field("parameter_two")
        .as_array()
        .ok_or("fence.parameter_two must be node array")?
        .iter()
        .map(|v| child(v, f, ctx))
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
fn unary_slot_text(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if !present(v) {
        return Ok(String::new());
    }
    if let Some(a) = v.as_array() {
        let sep = if f == "latex" { " " } else { "" };
        return a
            .iter()
            .filter(|v| !v.is_null())
            .map(|v| child(v, f, ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join(sep));
    }
    child(v, f, ctx)
}
fn unary_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let name = m.kind().to_lowercase();
    let v = m.field("parameter_one");
    if f == "unicodemath" && !v.is_null() && !v.is_object() {
        return Err("source_render_error: Unicode unary argument must be a node; source calls to_unicodemath directly".into());
    }
    let body = unary_slot_text(v, f, ctx)?;
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
fn unary_slot_xml(v: &Value, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if !present(v) {
        return Ok(vec![]);
    }
    if let Some(a) = v.as_array() {
        let mut out = vec![];
        for v in a.iter().filter(|v| !v.is_null()) {
            out.extend(render_xml(&Model::from_value(v)?, f, ctx)?)
        }
        Ok(out)
    } else {
        render_xml(&Model::from_value(v)?, f, ctx)
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
fn unary_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let name = m.kind().to_lowercase();
    let param = m.field("parameter_one");
    let hide = present(m.field("hide_function_name"));
    let body = unary_slot_xml(param, f, ctx)?;
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
            vec![attrs(
                "mrow",
                if ctx.intent {
                    vec![("intent".into(), ":function".into())]
                } else {
                    vec![]
                },
                c,
            )]
        } else {
            c
        };
        if mi && ctx.unary_function_spacing {
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
fn parens(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if v.is_null() || !v.is_object() {
        return Err("source_render_error: argument must receive its source node writer".into());
    }
    let out = child(v, f, ctx)?;
    Ok(if v["class"] == "Math::Function::Fenced" {
        out
    } else {
        format!("({out})")
    })
}
fn special_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let v = m.field("parameter_one");
    match(m.kind(),f){
 ("Phantom","asciimath")=>Ok("\\ ".repeat(unary_slot_text(v,f, ctx)?.chars().count())),
 ("Phantom","html")=>Err("source_render_error: Phantom#to_html accepts no options keyword at the formula writer boundary".into()),
 ("Phantom","unicodemath")=>{
 if v["class"]=="Math::Function::Mpadded" {
  use crate::source_values::{field,Domain,RubyValue};
  let child=Model::from_value(v)?;let options=field(&child,"options",Domain::NamedOptions)?;
  if !matches!(options,RubyValue::Nil|RubyValue::Hash(_)){return Err("source_render_error: Mpadded options have no dig method".into());}
  if options.get_symbol("phantom").is_some_and(|v|!matches!(v,RubyValue::Nil|RubyValue::Bool(false))){return Ok(format!("{}{}",layout_nodes::phantom_prefix(&child)?,parens(child.field("parameter_one"),f,ctx)?));}
 }
 Ok(format!("⟡{}",parens(v,f,ctx)?))
 },
 ("Cancel","unicodemath")=>Ok(format!("╱{}",if present(v){parens(v,f, ctx)?}else{String::new()})),
 _=>unary_text(m,f, ctx)}
}
fn strict_slot_xml(
    v: &Value,
    f: &str,
    ctx: &WriterContext,
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
            c.extend(render_xml(&Model::from_value(v)?, f, ctx)?)
        }
        return Ok(c);
    }
    if !v.is_object() {
        return Err("source_render_error: XML argument is not a node".into());
    }
    render_xml(&Model::from_value(v)?, f, ctx)
}
fn special_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let v = m.field("parameter_one");
    let body = strict_slot_xml(v, f, ctx, m.kind() == "Phantom" || f == "omml")?;
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
fn prime_symbol(v: &Value, _ctx: &WriterContext) -> Result<bool, String> { final_options::source_prime(v) }

fn power_accent(v: &Value, ctx: &WriterContext) -> Result<bool, String> {
    let Ok(m) = Model::from_value(v) else {
        return Ok(false);
    };
    if m.kind() == "Formula" {
        return prime_symbol(&m.field("value")[0], ctx);
    }
    if m.kind() == "Power" {
        return prime_symbol(m.field("parameter_one"), ctx);
    }
    prime_symbol(v, ctx)
}
fn power_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let a = m.field("parameter_one");
    let b = m.field("parameter_two");
    match f {
        "asciimath" => {
            if a.is_null() {
                return Err("source_render_error: power base cannot receive to_asciimath".into());
            }
            let base = child(a, f, ctx)?;
            Ok(if present(b) {
                format!("{base}^({})", child(b, f, ctx)?)
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
                child(a, f, ctx)?,
                if present(b) {
                    child(b, f, ctx)?
                } else {
                    String::new()
                }
            ))
        }
        "html" => Ok(format!(
            "{}{}",
            if present(a) {
                let value = child(a, f, ctx)?;
                if a["class"] == "Math::Function::FontStyle::Normal" {
                    value
                } else {
                    format!("<i>{value}</i>")
                }
            } else {
                String::new()
            },
            if present(b) {
                format!("<sup>{}</sup>", child(b, f, ctx)?)
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
            let base = child(a, f, ctx)?;
            if power_accent(b, ctx)? {
                return Ok(format!(
                    "{base}{}",
                    child(b, f, ctx)?
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace())
                        .collect::<String>()
                ));
            }
            Ok(format!(
                "{base}^{}",
                if b["class"] == "Math::Function::Power" {
                    child(b, f, ctx)?
                } else {
                    parens(b, f, ctx)?
                }
            ))
        }
        _ => unreachable!(),
    }
}
fn power_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if f == "mathml"
        && [m.field("parameter_one"), m.field("parameter_two")]
            .iter()
            .any(|value| value.is_array())
    {
        return Err("source_render_error: array cannot receive to_mathml_without_math_tag".into());
    }
    let a = if f == "omml" && !present(m.field("parameter_one")) {
        vec![run("&#8203;")]
    } else {
        strict_slot_xml(m.field("parameter_one"), f, ctx, true)?
    };
    let b = if f == "omml" && !present(m.field("parameter_two")) {
        vec![run("&#8203;")]
    } else {
        strict_slot_xml(m.field("parameter_two"), f, ctx, true)?
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
                && text_content::decode(value).is_ok_and(|s| s.chars().count() > 1)
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
fn normal_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let p = m.field("parameter_one");
    if !p.is_null() && !p.is_object() {
        return Err(
            "source_render_error: Normal argument cannot receive its source node writer".into(),
        );
    }
    let s = child(p, f, ctx)?;
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
fn normal_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
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
        render_xml(&child, f, ctx)?
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
fn render_text(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
 if m.class=="Math::Function::Uncertainty" {return uncertainty_writers::text_writer(m,f,ctx);}
    crate::source_values::writer_admission(m)?;
    let scoped;
    let ctx=if m.class=="Math::Formula"||m.class.starts_with("Math::Formula::") {scoped=ctx.with_formula(m);&scoped}else{ctx};
    if ["Math::Function::Intent","Math::Function::Arg"].contains(&m.class.as_str()) { return intent_nodes::text(m,f,ctx); }
    if leaf_nodes::handles(m) {return leaf_nodes::text_writer(m,f,ctx);}
    if underline::handles(m) {return underline::text_writer(m,f,ctx)}
    if table_text::handles(m) {return table_text::render(m,f,ctx)}
    if m.class=="Math::Symbols::Comma"&&f=="asciimath"&&ctx.table {return Ok("\",\"".into())}
    if portable_functions::handles(m) {return portable_functions::text_writer(m,f,ctx);}
    if mod_log::handles(m) {return mod_log::text_writer(m,f,ctx);}
    if m.class=="Math::Function::None" {return multiscripts::child(&m.value(),f,ctx);}
    if final_options::handles(m) {return final_options::text_writer(m,f,ctx);}
    if layout_nodes::handles(m) { return layout_nodes::text_writer(m, f, ctx); }
    if fences::handles(m) { return fences::text(m, f, ctx); }
    if m.class == "Math::Function::Nary" { return generic_nary::text_writer(m, f, ctx); }
    if numbers::handles(m) { return numbers::text(m, f, ctx); }
    if ["Math::Function::Multiscript", "Math::Function::Linebreak"].contains(&m.class.as_str()) { return multiscripts::text(m, f, ctx); }
    if stacked::handles_text(m) { return stacked::text(m, f, ctx); }
    if m.class == "Math::Function::PowerBase" { return operators::text_writer(m,f, ctx); }
    if accents::handles(m) {
        return accents::text(m, f, ctx);
    }
    if m.kind() == "Color" {
        return styles::text(m, f, ctx);
    }
    if m.kind() == "Base" {
        return scripts::text(m, f, ctx);
    }
    if m.class.starts_with("Math::Function::Table::") {
        return tables::render(m, f, ctx);
    }
    if m.kind() == "Unitsml" {
        let model = derived_units(m)?;
        if f == "unicodemath" {
            let mut v = model.value();
            unit_unicode_context(&mut v);
            return render_text(&Model::from_value(&v)?, f, ctx);
        }
        return render_text(&model, f, ctx);
    }
    if m.class == "Math::Function::FontStyle::Normal" {
        return normal_text(m, f, ctx);
    }
    if font_styles::handles(m) { return font_styles::text(m, f, ctx); }
    if nary::handles(m) { return nary::text(m, f, ctx); }
    if ["Text", "Mbox", "Left", "Right"].contains(&m.kind()) {
        return text_content::text_writer(m, f);
    }
    if m.kind() == "Power" {
        return power_text(m, f, ctx);
    }
    if ["Phantom", "Cancel"].contains(&m.kind()) {
        return special_text(m, f, ctx);
    }
    if unary_class(m) {
        return unary_text(m, f, ctx);
    }
    if m.class.starts_with("Math::Symbols::") {
        return symbol(m, f);
    }
    match m.kind() {
        "Fenced" => fence_text(m, f, ctx),
        "Formula" | "Mrow" | "Mstyle" => {
            let c = children(m)?
                .iter()
                .map(|v| child(v, f, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            let joined = c.join(if f == "unicodemath" && m.kind() == "Formula" { if fences::formula_starts_mini_fence(m) { "" } else { numbers::unicode_separator(m) } } else { " " });
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
            let a = child(m.field("parameter_one"), f, ctx)?;
            let b = child(m.field("parameter_two"), f, ctx)?;
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
        _ => templates::text_template(m, f, ctx)
            .unwrap_or_else(|| Err(format!("unported promised writer node {}", m.class))),
    }
}
fn omml_slot(value: &Value, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if !present(value) {
        return Ok(vec![run("&#8203;")]);
    }
    if value
        .as_array()
        .is_some_and(|nodes| nodes.iter().any(Value::is_null))
    {
        return Err("source_render_error: nil array entry cannot receive insert_t_tag".into());
    }
    strict_slot_xml(value, format, ctx, true)
}
fn abs_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let value = m.field("parameter_one");
    if f == "mathml" {
        let mut content = unary_slot_xml(value, f, ctx)?;
        if !present(m.field("open_paren")) {
            content.insert(0, el("mo", vec![text("|")]));
        }
        if !present(m.field("close_paren")) {
            content.push(el("mo", vec![text("|")]));
        }
        return Ok(vec![el("mrow", content)]);
    }
    let mut props = vec![word_properties(false)];
    if !present(m.field("open_paren")) {
        props.push(attrs(
            "m:begChr",
            vec![("m:val".into(), "|".into())],
            vec![],
        ));
    }
    if !present(m.field("close_paren")) {
        props.push(attrs(
            "m:endChr",
            vec![("m:val".into(), "|".into())],
            vec![],
        ));
    }
    props.push(attrs("m:sepChr", vec![("m:val".into(), "".into())], vec![]));
    props.push(el("m:grow", vec![]));
    Ok(vec![el(
        "m:d",
        vec![el("m:dPr", props), el("m:e", omml_slot(value, f, ctx)?)],
    )])
}
fn overset_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let first = m.field("parameter_one");
    let second = m.field("parameter_two");
    if f == "mathml" {
        if [first, second].iter().any(|value| {
            value
                .as_array()
                .is_some_and(|values| values.iter().any(Value::is_null))
        }) {
            return Err("source_render_error: nil array entry cannot receive mathml_nodes".into());
        }
        let mut content = strict_slot_xml(second, f, ctx, true)?;
        content.extend(strict_slot_xml(first, f, ctx, true)?);
        let a = match m.field("options") {
            Value::Null => vec![],
            Value::Object(values) => values
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect(),
            _ => return Err("source_render_error: overset options must support attributes".into()),
        };
        return Ok(vec![attrs("mover", a, content)]);
    }
    if !ctx.display_style {
        let power = crate::model::node(
            "Math::Function::Power",
            serde_json::json!({"parameter_one":second,"parameter_two":first}),
        );
        return render_xml(&Model::from_value(&power)?, f, ctx);
    }
    Ok(vec![el(
        "m:limUpp",
        vec![
            el("m:limUppPr", vec![control_properties()]),
            el("m:e", omml_slot(second, f, ctx)?),
            el("m:lim", omml_slot(first, f, ctx)?),
        ],
    )])
}

fn named_limit_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let lower = m.field("parameter_one");
    let upper = m.field("parameter_two");
    let name = m.kind().to_lowercase();
    if !present(lower) && !present(upper) {
        return Ok(if f == "mathml" {
            vec![el("mo", vec![text(&name)])]
        } else {
            vec![run(&name)]
        });
    }
    if f == "mathml" {
        let mut content = vec![el("mo", vec![text(&name)])];
        content.extend(strict_slot_xml(lower, f, ctx, false)?);
        content.extend(strict_slot_xml(upper, f, ctx, false)?);
        let tag = if present(lower) && present(upper) {
            "munderover"
        } else if present(lower) {
            "munder"
        } else {
            "mover"
        };
        return Ok(vec![el(tag, content)]);
    }
    let base = crate::model::node("Math::Symbols::Symbol", serde_json::json!({"value":name}));
    if !ctx.display_style {
        return render_xml(
            &Model::from_value(&crate::model::node(
                "Math::Function::PowerBase",
                serde_json::json!({"parameter_one":base,"parameter_two":lower,"parameter_three":upper}),
            ))?,
            f,
            ctx,
        );
    }
    let overset = Model::from_value(&crate::model::node(
        "Math::Function::Overset",
        serde_json::json!({"parameter_one":upper,"parameter_two":base}),
    ))?;
    let top = overset_xml(&overset, f, ctx)?;
    if !present(lower) {
        return Ok(top);
    }
    Ok(vec![el(
        "m:limLow",
        vec![
            el("m:limLowPr", vec![control_properties()]),
            el("m:e", top),
            el("m:lim", omml_slot(lower, f, ctx)?),
        ],
    )])
}
fn underover_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if f == "mathml" {
        let mut content = vec![];
        for key in ["parameter_one", "parameter_two", "parameter_three"] {
            content.extend(strict_slot_xml(m.field(key), f, ctx, true)?);
        }
        return Ok(vec![el("munderover", content)]);
    }
    if m.kind() == "Underover" && !ctx.display_style {
        return render_xml(
            &Model::from_value(&crate::model::node(
                "Math::Function::PowerBase",
                serde_json::json!({"parameter_one":m.field("parameter_one"),"parameter_two":m.field("parameter_two"),"parameter_three":m.field("parameter_three")}),
            ))?,
            f,
            ctx,
        );
    }
    let overset = Model::from_value(&crate::model::node(
        "Math::Function::Overset",
        serde_json::json!({"parameter_one":m.field("parameter_three"),"parameter_two":m.field("parameter_one")}),
    ))?;
    if !present(m.field("parameter_two")) {
        return overset_xml(&overset, f, ctx);
    }
    if !ctx.display_style {
        // Limits invokes Underset with the caller's style, unlike Underover's
        // direct PowerBase branch. Reuse Base, preserving that source boundary.
        let lower = crate::model::node(
            "Math::Function::Base",
            serde_json::json!({"parameter_one":overset.value(),"parameter_two":m.field("parameter_two")}),
        );
        return render_xml(&Model::from_value(&lower)?, f, ctx);
    }
    Ok(vec![el(
        "m:limLow",
        vec![
            el("m:limLowPr", vec![control_properties()]),
            el("m:e", overset_xml(&overset, f, ctx)?),
            el("m:lim", omml_slot(m.field("parameter_two"), f, ctx)?),
        ],
    )])
}
fn contains_xml_attr(node: &Xml, name: &str, value: &str) -> bool {
    xml_attr(node, name) == Some(value)
        || xml_nodes(node)
            .iter()
            .any(|child| contains_xml_attr(child, name, value))
}
fn update_partial_arguments(nodes: &mut [Xml]) {
    for index in 0..nodes.len().saturating_sub(1) {
        if xml_name(&nodes[index]) != "mfrac"
            || !xml_attr(&nodes[index], "intent")
                .is_some_and(|value| value.starts_with(":partial-derivative"))
            || contains_xml_attr(&nodes[index], "arg", "f")
        {
            continue;
        }
        if matches!(xml_name(&nodes[index + 1]), "mfrac" | "mo") {
            let intent = xml_attr(&nodes[index], "intent").unwrap().replace("$f", "");
            xml_attribute(&mut nodes[index], "intent", intent);
        } else {
            xml_attribute(&mut nodes[index + 1], "arg", "f".into());
        }
    }
}

fn radical_mathml(m: &Model, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    Ok(vec![el("msqrt", strict_slot_xml(m.field("parameter_one"), "mathml", ctx, false)?)])
}
fn delimiter_unary_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let kind = m.kind();
    let value = m.field("parameter_one");
    let (left, right) = match kind {
        "Ceil" => ("&#x2308;", "&#x2309;"),
        "Floor" => ("&#x230a;", "&#x230b;"),
        "Norm" => ("&#x2225;", "&#x2225;"),
        _ => unreachable!(),
    };
    if f == "mathml" {
        let mut content = if kind == "Ceil" {
            unary_slot_xml(value, f, ctx)?
        } else {
            strict_slot_xml(value, f, ctx, false)?
        };
        if !present(m.field("open_paren")) {
            content.insert(0, el("mo", vec![text(left)]));
        }
        if !present(m.field("close_paren")) {
            content.push(el("mo", vec![text(right)]));
        }
        return Ok(vec![el("mrow", content)]);
    }
    let body = unary_slot_xml(value, f, ctx)?;
    let left = text_content::decode(left)?;
    let right = text_content::decode(right)?;
    if kind == "Ceil" {
        let mut props = vec![];
        if !present(m.field("open_paren")) {
            props.push(attrs("m:begChr", vec![("m:val".into(), left)], vec![]));
        }
        props.push(attrs("m:sepChr", vec![("m:val".into(), "".into())], vec![]));
        if !present(m.field("close_paren")) {
            props.push(attrs("m:endChr", vec![("m:val".into(), right)], vec![]));
        }
        return Ok(vec![el("m:d", vec![el("m:dPr", props), el("m:e", body)])]);
    }
    let mark = |value: &str| {
        el(
            "m:r",
            vec![
                el(
                    "m:rPr",
                    vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
                ),
                el("m:t", vec![text(value)]),
            ],
        )
    };
    let mut content = vec![];
    if !present(m.field("open_paren")) {
        content.push(mark(&left));
    }
    content.extend(body);
    if !present(m.field("close_paren")) {
        content.push(mark(&right));
    }
    Ok(content)
}

fn render_xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let mut nodes = render_xml_inner(m, f, ctx)?;
    if f == "mathml" && ctx.intent {
        apply_writer_intent(m, &mut nodes)?;
    }
    Ok(nodes)
}
fn render_xml_inner(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
 if m.class=="Math::Function::Uncertainty" {return uncertainty_writers::xml(m,f,ctx);}
    crate::source_values::writer_admission(m)?;
    if ["Math::Function::Intent","Math::Function::Arg"].contains(&m.class.as_str()) { return intent_nodes::xml(m,f,ctx); }
    if leaf_nodes::handles(m) {return leaf_nodes::xml(m,f,ctx);}
    if underline::handles(m) {return underline::xml(m,f,ctx)}
    if matches!(m.kind(),"Stackrel"|"Obrace"|"Sqrt") {return portable_functions::xml(m,f,ctx);}
    if m.class=="Math::Function::Log" {return mod_log::log_xml(m,f,ctx);}
    if m.class=="Math::Function::None" {return multiscripts::xml_child(&m.value(),f,ctx);}
    if final_options::handles(m) {return final_options::xml(m,f,ctx);}
    if layout_nodes::handles(m) { return layout_nodes::xml(m, f, ctx); }
    if m.class == "Math::Function::Mod" { return line_break_overrides::mod_xml(m, f, ctx); }
    if fences::handles(m) { return fences::xml(m, f, ctx); }
    if m.class == "Math::Formula" && f == "mathml" && ctx.intent
        && present(m.field("left_right_wrapper"))
        && m.field("value").as_array().is_some_and(Vec::is_empty)
    {
        return Err("source_render_error: empty Formula has no first parameter for intent".into());
    }
    if m.class == "Math::Function::Nary" { return generic_nary::xml(m, f, ctx); }
    if numbers::handles(m) { return numbers::xml(m, f, ctx); }
    if ["Math::Function::Multiscript", "Math::Function::Linebreak"].contains(&m.class.as_str()) { return multiscripts::xml(m, f, ctx); }
    if ["Substack", "Msline", "Mglyph"].contains(&m.kind()) { return stacked::xml(m, f, ctx); }
    if nary::handles(m) { return nary::xml(m, f, ctx); }
    if m.class == "Math::Function::PowerBase" || (f == "omml" && ["Math::Function::Mod", "Math::Function::Sqrt"].contains(&m.class.as_str())) { return operators::xml_with_style(m, f, ctx, Some(ctx.display_style)); }
    if m.class == "Math::Function::Sqrt" && f == "mathml" {
        return radical_mathml(m, ctx);
    }
    if matches!(
        m.class.as_str(),
        "Math::Function::Ceil" | "Math::Function::Floor" | "Math::Function::Norm"
    ) {
        return delimiter_unary_xml(m, f, ctx);
    }
    if m.class == "Math::Function::Abs" {
        return abs_xml(m, f, ctx);
    }
    if m.class == "Math::Function::Overset" {
        return overset_xml(m, f, ctx);
    }
    if m.kind() == "Linebreak" {
        let value = m.field("parameter_one");
        if !present(value) {
            return Ok(if f == "mathml" {
                vec![attrs(
                    "mo",
                    vec![("linebreak".into(), "newline".into())],
                    vec![],
                )]
            } else {
                vec![]
            });
        }
        let mut nodes = strict_slot_xml(value, f, ctx, false)?;
        if f == "mathml" {
            if nodes.len() != 1 {
                return Err(
                    "source_render_error: linebreak parameter must produce one element".into(),
                );
            }
            if let Xml::El(name, attributes, _) = &mut nodes[0] {
                *name = "mo".into();
                if let Some(values) = m.field("attributes").as_object() {
                    for (key, value) in values {
                        attributes.push((
                            key.clone(),
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string()),
                        ));
                    }
                }
            }
        }
        return Ok(nodes);
    }
    if matches!(
        m.class.as_str(),
        "Math::Function::Lim" | "Math::Function::Inf"
    ) {
        return named_limit_xml(m, f, ctx);
    }
    if matches!(
        m.class.as_str(),
        "Math::Function::Limits" | "Math::Function::Underover"
    ) {
        return underover_xml(m, f, ctx);
    }
    if accents::handles(m) {
        return accents::xml(m, f, ctx);
    }
    if m.kind() == "Color" {
        return styles::xml(m, f, ctx);
    }
    if m.kind() == "Base" {
        return scripts::xml(m, f, ctx);
    }
    if matches!(
        m.class.as_str(),
        "Math::Function::Table"
            | "Math::Function::Table::Matrix"
            | "Math::Function::Table::Pmatrix"
            | "Math::Function::Table::Bmatrix"
            | "Math::Function::Table::Vmatrix"
            | "Math::Function::Table::Array"
            | "Math::Function::Table::Cases"
            | "Math::Function::Table::Eqarray"
            | "Math::Function::Tr"
            | "Math::Function::Td"
    ) {
        return table_xml::render(m, f, ctx);
    }

    if m.kind() == "Unitsml" {
        return render_xml(&derived_units(m)?, f, ctx);
    }
    if m.class == "Math::Function::FontStyle::Normal" {
        return normal_xml(m, f, ctx);
    }
    if font_styles::handles(m) {
        return font_styles::xml(m, f, ctx);
    }

    if ["Text", "Mbox", "Left", "Right"].contains(&m.kind()) {
        return text_content::xml_writer(m, f);
    }
    if m.kind() == "Power" {
        return power_xml(m, f, ctx);
    }
    if ["Phantom", "Cancel"].contains(&m.kind()) {
        return special_xml(m, f, ctx);
    }
    if unary_class(m) {
        return unary_xml(m, f, ctx);
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
            let encoded = match m.kind() {
                "Ii" => Some("ⅈ"),
                "Jj" => Some("ⅉ"),
                "Dd" => Some("ⅆ"),
                "UpcaseDd" => Some("ⅅ"),
                "Intercal" => Some("transpose"),
                _ => None,
            };
            attrs(
                &tag,
                if ctx.intent {
                    encoded
                        .map(|value| vec![("intent".into(), value.into())])
                        .unwrap_or_default()
                } else {
                    vec![]
                },
                vec![text(&s)],
            )
        } else {
            el("m:r", vec![el("m:t", vec![text(&s)])])
        }]);
    }
    match m.kind() {
        "Fenced" => {
            let delimiter = |key: &str| -> Result<String, String> {
                let value = m.field(key);
                if value.is_null() {
                    return Ok(String::new());
                }
                if !value.is_object() {
                    return Err(
                        "source_render_error: raw fence delimiter cannot receive its writer".into(),
                    );
                }
                fence_delimiter(value, f, ctx)
            };
            let a = delimiter("parameter_one")?;
            let b = delimiter("parameter_three")?;
            let options = m.field("options");
            if !options.is_null() && options.as_object().is_none_or(|o| !o.is_empty()) {
                return Err("unported promised fence options".into());
            }
            let mut body = vec![];
            let values = if m.field("parameter_two").is_null() {
                &[][..]
            } else {
                m.field("parameter_two")
                    .as_array()
                    .ok_or("source_render_error: fence parameter must support map")?
                    .as_slice()
            };
            for v in values {
                if v.is_null() {
                    if f == "mathml" {
                        continue;
                    }
                    return Err(
                        "source_render_error: nil fence child cannot receive insert_t_tag".into(),
                    );
                }
                body.extend(render_xml(&Model::from_value(v)?, f, ctx)?)
            }
            if f == "mathml" {
                if ctx.intent {
                    update_partial_arguments(&mut body);
                }
                let mut c = vec![el("mo", vec![text(&a)])];
                c.extend(body);
                c.push(el("mo", vec![text(&b)]));
                Ok(vec![el("mrow", c)])
            } else {
                let mut props = vec![];
                let has_delimiter = |v: &Value| {
                    v["class"]
                        .as_str()
                        .is_some_and(|c| c.starts_with("Math::Symbols::Paren::"))
                        || !v["fields"]["value"].is_null()
                };
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
                if f == "mathml" && ctx.intent && source_formula_array(&v) {
                    return Err(intent_error());
                }
                c.extend(render_xml(&Model::from_value(&v)?, f, ctx)?)
            }
            if f == "mathml" && ctx.intent {
                formula_postprocess(&mut c)?;
            }
            if f == "mathml"
                && m.field("left_right_wrapper") != false
                && !m.field("left_right_wrapper").is_null()
            {
                let a = if ctx.intent && formula_function_intent(m) {
                    vec![("intent".into(), ":function".into())]
                } else {
                    vec![]
                };
                Ok(vec![attrs("mrow", a, c)])
            } else {
                Ok(c)
            }
        }
        "Number" => {
            let s = render_text(m, "asciimath", ctx)?;
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
        "Frac" | "Over" => {
            if m.kind() == "Frac" && !m.field("options").is_null() {
                return Err("fraction option profile not yet ported".into());
            }
            let slot = |key: &str| -> Result<Vec<Xml>, String> {
                let v = m.field(key);
                if f == "omml" {
                    omml_slot(v, f, ctx)
                } else if v.is_null() {
                    Ok(vec![])
                } else {
                    strict_slot_xml(v, f, ctx, false)
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
        _ => templates::xml_template(m, f, ctx)
            .unwrap_or_else(|| Err(format!("unported promised writer node {}", m.class))),
    }
}
fn formula_function_intent(m: &Model) -> bool {
    let Some(first) = m
        .field("value")
        .as_array()
        .and_then(|values| values.first())
    else {
        return false;
    };
    let Ok(first) = Model::from_value(first) else {
        return false;
    };
    if first.class == "Math::Symbols::Symbol" {
        // UnicodeMath::Constants::UNDEF_UNARY_FUNCTIONS is source-generated data.
        return matches!(
            first.field("value").as_str(),
            Some("arg" | "def" | "erf" | "Im" | "Pr" | "Re" | "tg")
        );
    }
    if !matches!(first.kind(), "Power" | "Base" | "PowerBase") {
        return false;
    }
    let Ok(base) = Model::from_value(first.field("parameter_one")) else {
        return false;
    };
    source_class_properties(&base).2 || font_styles::handles(&base) || base.kind() == "Lim"
}

fn xml_name(node: &Xml) -> &str {
    match node {
        Xml::El(name, _, _) => name,
        Xml::Text(_) => "",
    }
}
fn xml_nodes(node: &Xml) -> &[Xml] {
    match node {
        Xml::El(_, _, nodes) => nodes,
        Xml::Text(_) => &[],
    }
}
fn xml_nodes_mut(node: &mut Xml) -> Result<&mut Vec<Xml>, String> {
    match node {
        Xml::El(_, _, nodes) => Ok(nodes),
        _ => Err("source_render_error: intent expected an element".into()),
    }
}
fn xml_first_text(node: &Xml) -> &str {
    match xml_nodes(node).first() {
        Some(Xml::Text(value)) => value,
        _ => "",
    }
}
fn xml_attribute(node: &mut Xml, key: &str, value: String) {
    if let Xml::El(_, attributes, _) = node {
        if let Some((_, old)) = attributes.iter_mut().find(|(name, _)| name == key) {
            *old = value;
        } else {
            attributes.push((key.into(), value));
        }
    }
}
fn intent_node_value(node: &mut Xml, field: &str) -> Result<String, String> {
    let name = xml_name(node);
    if name == "mo" && xml_first_text(node) == "&#x2b1a;" {
        return Ok(String::new());
    }
    if matches!(name, "mi" | "mo" | "mn") || name == "mrow" && xml_nodes(node).is_empty() {
        return text_content::decode(xml_first_text(node));
    }
    if name == "mrow" && xml_nodes(node).iter().all(|child| xml_name(child) == "mn") {
        return Ok(xml_nodes(node).iter().map(xml_first_text).collect());
    }
    xml_attribute(node, "arg", field.into());
    Ok(format!("${field}"))
}
fn derivative_symbol(node: &Xml) -> bool {
    let value = xml_first_text(node);
    value == "d" || value.contains("&#x2146;") || value.contains("&#x2145;")
}
fn valid_derivative(node: &Xml) -> bool {
    if matches!(xml_name(node), "mi" | "mo" | "mn") {
        return derivative_symbol(node);
    }
    xml_nodes(node)
        .first()
        .is_some_and(|child| xml_name(child) == "mi" && derivative_symbol(child))
        && xml_nodes(node).len() >= 2
}
fn partial_fraction_field(node: &Xml) -> Result<bool, String> {
    match xml_name(node) {
        "mo" => Ok(xml_first_text(node) == "&#x2202;"),
        "mrow" | "msup" | "msubsup" => {
            partial_fraction_field(xml_nodes(node).first().ok_or_else(intent_error)?)
        }
        _ => Ok(false),
    }
}
fn partial_order(node: &Xml) -> Result<String, String> {
    let power = if matches!(xml_name(node), "msup" | "msubsup") {
        Some(node)
    } else {
        let mut found = None;
        for child in xml_nodes(node) {
            if matches!(child, Xml::Text(_)) {
                return Err(intent_error());
            }
            if matches!(xml_name(child), "msup" | "msubsup") {
                found = Some(child);
                break;
            }
        }
        found
    };
    let Some(power) = power else {
        return Ok("1".into());
    };
    let exponent = xml_nodes(power).get(1).ok_or_else(intent_error)?;
    if matches!(xml_name(exponent), "mo" | "mi" | "mn") {
        text_content::decode(xml_first_text(exponent))
    } else {
        Ok("$n".into())
    }
}
fn partial_function(node: &mut Xml) -> Result<String, String> {
    let script = matches!(xml_name(node), "msup" | "msubsup");
    let children = xml_nodes_mut(node)?;
    let after = children.get(1..).ok_or_else(intent_error)?;
    if after.iter().all(|child| xml_name(child) == "mi") {
        return text_content::decode(after.first().map(xml_first_text).unwrap_or(""));
    }
    if !after.is_empty() && !script {
        let rest = children.split_off(1);
        children.push(attrs("mrow", vec![("arg".into(), "$f".into())], rest));
    }
    Ok("$f".into())
}
fn partial_variables(node: &Xml, siblings: &[Xml]) -> Result<String, String> {
    match xml_name(node) {
        "mo" => {
            let after = siblings.get(1..).ok_or_else(intent_error)?;
            let mut variables = vec![];
            for child in after {
                match xml_name(child) {
                    "mi" => variables.push(text_content::decode(xml_first_text(child))?),
                    "msup" | "msubsup" => variables.push(text_content::decode(xml_first_text(
                        xml_nodes(child).first().ok_or_else(intent_error)?,
                    ))?),
                    _ => {}
                }
            }
            Ok(variables.join(","))
        }
        "msup" => {
            let children = xml_nodes(node);
            let base = children.first().ok_or_else(intent_error)?;
            let mut variables = vec![];
            for child in xml_nodes(base) {
                if xml_first_text(child) == "&#x2202;" {
                    continue;
                }
                if matches!(child, Xml::Text(_)) {
                    return Err(intent_error());
                }
                variables.push(text_content::decode(xml_first_text(child))?);
            }
            let exponent = children.get(1).ok_or_else(intent_error)?;
            if prime_node(exponent) {
                variables
                    .last_mut()
                    .ok_or_else(intent_error)?
                    .push_str(xml_first_text(exponent));
            }
            text_content::decode(&variables.join(","))
        }
        "mrow" => partial_variables(
            xml_nodes(node).first().ok_or_else(intent_error)?,
            xml_nodes(node),
        ),
        _ => Ok(String::new()),
    }
}
fn update_fraction_derivative(m: &Model, node: &mut Xml) -> Result<(), String> {
    let children = xml_nodes_mut(node)?;
    let num = children.first().ok_or_else(intent_error)?;
    if matches!(xml_name(num), "mi" | "mo" | "mn") {
        return Ok(());
    }
    let intent = xml_nodes(num)
        .first()
        .and_then(|child| xml_attr(child, "intent"))
        .unwrap_or("")
        .to_owned();
    if !intent.starts_with(":derivative") || !intent.ends_with(",)") {
        return Ok(());
    }
    if m.field("parameter_one")["class"] == "Math::Formula"
        && m.field("parameter_one")["fields"]["left_right_wrapper"] == false
    {
        return Err(intent_error());
    }
    let den = children.get(1).ok_or_else(intent_error)?;
    let den_nodes = xml_nodes(den);
    let first = den_nodes.first().ok_or_else(intent_error)?;
    let mut variable = String::new();
    if xml_name(first) == "mi" {
        let script = den_nodes.get(1).ok_or_else(intent_error)?;
        if matches!(xml_name(script), "msub" | "msup") {
            let base = xml_nodes(script).first().ok_or_else(intent_error)?;
            if xml_name(base) == "mi" {
                variable = text_content::decode(xml_first_text(base))?;
            } else if xml_name(base) == "mrow" {
                for child in xml_nodes(base) {
                    if xml_name(child) != "mi" {
                        break;
                    }
                    variable.push_str(&text_content::decode(xml_first_text(child))?);
                }
            }
        }
    }
    let first = xml_nodes_mut(&mut children[0])?
        .first_mut()
        .ok_or_else(intent_error)?;
    // Source replaces the final comma and ')' together.
    xml_attribute(
        first,
        "intent",
        format!("{},{variable})", intent.trim_end_matches(",)")),
    );
    Ok(())
}

fn fraction_derivative_intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    update_fraction_derivative(m, node)?;
    let nodes = xml_nodes_mut(node)?;
    if nodes.is_empty() {
        return Err(intent_error());
    }
    let partial = partial_fraction_field(&nodes[0])?;
    if partial && partial_fraction_field(nodes.get(1).ok_or_else(intent_error)?)? {
        let order = partial_order(&nodes[0])?;
        let function = partial_function(&mut nodes[0])?;
        let variables = partial_variables(&nodes[1], &[])?;
        xml_attribute(
            node,
            "intent",
            format!(":partial-derivative({order},{function},{variables})"),
        );
        return Ok(());
    }
    if !valid_derivative(&nodes[0]) {
        return Ok(());
    }
    if !valid_derivative(nodes.get(1).ok_or_else(intent_error)?) {
        return Ok(());
    }
    let numerator = xml_nodes_mut(&mut nodes[0])?;
    let value = if numerator.len() <= 1 {
        return Err(intent_error());
    } else if matches!(xml_name(&numerator[1]), "mi" | "mo" | "mn") {
        text_content::decode(xml_first_text(&numerator[1]))?
    } else {
        xml_attribute(&mut numerator[1], "arg", "f".into());
        "$f".into()
    };
    let variable = xml_nodes(&nodes[1])
        .get(1)
        .map(xml_first_text)
        .unwrap_or("");
    let variable = text_content::decode(variable)?;
    xml_attribute(node, "intent", format!(":derivative(1,{value},{variable})"));
    Ok(())
}
fn apply_writer_intent(m: &Model, nodes: &mut [Xml]) -> Result<(), String> {
    let Some(node) = nodes.first_mut() else {
        return Ok(());
    };
    match m.kind() {
        "Abs" => {
            let child = xml_nodes_mut(node)?
                .get_mut(1)
                .ok_or("source_render_error: absolute value intent lacks its argument")?;
            let value = intent_node_value(child, "a")?;
            xml_attribute(node, "intent", format!("absolute-value({value})"));
        }
        "Lim" | "Inf" if present(m.field("parameter_one")) || present(m.field("parameter_two")) => {
            xml_attribute(node, "intent", ":function".into())
        }
        "Frac" => fraction_derivative_intent(m, node)?,
        "Fenced" => fences::intent(m, node)?,
        "Table" | "Matrix" | "Pmatrix" | "Bmatrix" | "Vmatrix" | "Array"
            if m.class.starts_with("Math::Function::Table") =>
        {
            table_intent(m, node)?
        }
        _ => {}
    }
    Ok(())
}
fn table_intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    let kind = m.kind();
    if kind == "Matrix" {
        return Ok(());
    }
    if kind == "Array" && m.field("options").is_null() {
        return Err("source_render_error: nil table attributes cannot receive merge".into());
    }
    if kind == "Table"
        && xml_name(node) == "mrow"
        && m.field("open_paren")["class"] != "Math::Symbols::Paren::Norm"
    {
        xml_attribute(node, "intent", ":fenced".into());
    }
    let row_count = m
        .field("value")
        .as_array()
        .map(Vec::len)
        .ok_or("source_render_error: table intent requires value.length")?;
    let column_count = m
        .field("value")
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["fields"]["parameter_one"].as_array())
        .map(|cells| cells.len().to_string())
        .unwrap_or_default();
    let inner_intent = if kind == "Array" {
        ":equations".into()
    } else {
        format!(":matrix({row_count},{column_count})")
    };
    if xml_name(node) == "mtable" {
        xml_attribute(node, "intent", inner_intent);
    } else {
        for child in xml_nodes_mut(node)? {
            if xml_name(child) == "mtable" {
                xml_attribute(child, "intent", inner_intent.clone());
                break;
            }
        }
    }
    let outer = match kind {
        "Pmatrix" => Some(":parenthesized-matrix"),
        "Bmatrix" => Some(
            if m.field("open_paren")["class"] == "Math::Symbols::Paren::Lcurly" {
                ":curly-braced-matrix"
            } else {
                ":bracketed-matrix"
            },
        ),
        "Vmatrix" => Some(
            if m.field("open_paren")["class"] == "Math::Symbols::Paren::Norm" {
                ":normed-matrix"
            } else {
                ":determinant"
            },
        ),
        _ => None,
    };
    if let Some(intent) = outer {
        xml_attribute(node, "intent", intent.into());
    }
    Ok(())
}
fn fenced_intent(m: &Model, node: &mut Xml) -> Result<(), String> {
    let children = xml_nodes_mut(node)?;
    let open = children
        .first()
        .map(xml_first_text)
        .unwrap_or("")
        .to_owned();
    let close = children.last().map(xml_first_text).unwrap_or("").to_owned();
    let valid_value = |n: &Xml| -> bool {
        let value = xml_first_text(n);
        match xml_name(n) {
            "mrow" => {
                xml_nodes(n).iter().all(|c| xml_name(c) == "mn")
                    || xml_nodes(n)
                        .iter()
                        .all(|c| matches!(xml_name(c), "mi" | "mo"))
            }
            "mi" | "mo" => value.chars().any(|c| c.is_ascii_alphabetic()),
            "mn" => value.chars().any(|c| c.is_ascii_digit()),
            _ => false,
        }
    };
    let logical_count = m
        .field("parameter_two")
        .as_array()
        .map(Vec::len)
        .unwrap_or(0)
        + 2;
    if logical_count == 5
        && m.field("parameter_two")
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.is_null() || source_formula_array(value))
    {
        return Err(intent_error());
    }
    let interval = if logical_count == 5
        && children.len() == 5
        && xml_first_text(&children[2]) == ","
        && valid_value(&children[1])
        && valid_value(&children[3])
    {
        match (open.as_str(), close.as_str()) {
            ("(", "]") | ("]", "]") => Some("open-closed-interval"),
            ("[", "]") => Some("closed-interval"),
            ("[", ")" | "[") => Some("closed-open-interval"),
            ("]", "[") => Some("open-interval"),
            _ => None,
        }
    } else {
        None
    };
    let intent = if let Some(name) = interval {
        let endpoint = |node: &mut Xml, field: &str| -> Result<String, String> {
            let children = xml_nodes(node);
            if children.len() == 2
                && matches!(xml_first_text(&children[0]), "&#x2212;" | "-" | "+")
                && xml_first_text(&children[1]) == "&#x221e;"
            {
                return text_content::decode(&format!(
                    "{}{}",
                    xml_first_text(&children[0]),
                    xml_first_text(&children[1])
                ));
            }
            intent_node_value(node, field)
        };
        let a = endpoint(&mut children[1], "a")?;
        let b = endpoint(&mut children[3], "b")?;
        format!("{name}({a},{b})")
    } else {
        ":fenced".into()
    };
    xml_attribute(node, "intent", intent);
    Ok(())
}

fn intent_error() -> String {
    "source_render_error: formula intent dereferenced a missing or non-element node".into()
}
fn derivative_constant(value: &str) -> bool {
    matches!(value, "&#x1d451;" | "&#x2145;" | "&#x2146;" | "d")
}
fn xml_attr<'a>(node: &'a Xml, key: &str) -> Option<&'a str> {
    match node {
        Xml::El(_, attributes, _) => attributes
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str()),
        _ => None,
    }
}
fn numeric_intent(node: &Xml) -> (String, String) {
    let value = xml_first_text(node);
    (
        value.chars().count().to_string(),
        value
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(","),
    )
}
fn prime_node(node: &Xml) -> bool {
    matches!(
        xml_first_text(node),
        "&#x2032;" | "&#x2033;" | "&#x2034;" | "&#x2057;" | "&#x27;"
    )
}
fn upcase_variable(nodes: &[Xml]) -> Result<String, String> {
    let mut value = String::new();
    for node in nodes {
        match xml_name(node) {
            "mi" => value.push_str(xml_first_text(node)),
            "mn" => value = xml_first_text(node).into(),
            "msub" | "msup" | "msubsup" => {
                let children = xml_nodes(node);
                let base = children.first().ok_or_else(intent_error)?;
                if !matches!(xml_name(base), "mi" | "mn") {
                    continue;
                }
                if xml_name(base) == "mn" {
                    return Err(intent_error());
                } // Source adds a node object to String.
                value.push_str(xml_first_text(base));
                let script = children
                    .get(if xml_name(node) == "msubsup" { 2 } else { 1 })
                    .ok_or_else(intent_error)?;
                if prime_node(script) {
                    value.push_str(xml_first_text(script));
                }
            }
            name => {
                if name == "mfrac" {
                    value.clear();
                }
                break;
            }
        }
    }
    text_content::decode(&value)
}
fn superscript_variable(nodes: &[Xml]) -> Result<String, String> {
    let mut fence = None;
    for node in nodes {
        if matches!(xml_name(node), "mi" | "mn") {
            continue;
        }
        if xml_name(node) == "mrow" && xml_attr(node, "intent") == Some(":fenced") {
            fence = Some(node);
        }
        break;
    }
    let Some(fence) = fence else {
        return Ok(String::new());
    };
    let children = xml_nodes(fence).get(1..).ok_or_else(intent_error)?;
    let start = children
        .iter()
        .rposition(|node| xml_name(node) == "mn")
        .unwrap_or(0);
    let mut value = String::new();
    for node in &children[start..] {
        match xml_name(node) {
            "msub" | "msup" | "msubsup" => {
                let children = xml_nodes(node);
                let base = children.first().ok_or_else(intent_error)?;
                value.push_str(&text_content::decode(xml_first_text(base))?);
                let script = children.last().ok_or_else(intent_error)?;
                if prime_node(script) {
                    value.push_str(&text_content::decode(xml_first_text(script))?);
                }
            }
            "mi" | "mn" => value.push_str(xml_first_text(node)),
            _ => break,
        }
    }
    Ok(value)
}
fn partial_subscript_intent(node: &Xml) -> Result<String, String> {
    let script = xml_nodes(node).last().ok_or_else(intent_error)?;
    let (order, variable) = match xml_name(script) {
        "mi" => ("1".into(), text_content::decode(xml_first_text(script))?),
        "mn" => return Err(intent_error()), // Source numeric branch references undefined `node`.
        "mrow" | "msup" => {
            let children = if xml_name(script) == "msup" {
                let base = xml_nodes(script).first().ok_or_else(intent_error)?;
                if xml_name(base) == "mi" {
                    return Err(intent_error());
                } else {
                    xml_nodes(base).to_vec()
                }
            } else {
                xml_nodes(script).to_vec()
            };
            let mut value = String::new();
            for child in &children {
                match xml_name(child) {
                    "mi" => value.push_str(&text_content::decode(xml_first_text(child))?),
                    "mn" => {
                        if value.is_empty() {
                            value.push_str(&numeric_intent(child).1);
                        }
                        break;
                    }
                    _ => {}
                }
            }
            let order = if value.contains(',') {
                value.split(',').count()
            } else {
                value.chars().count()
            };
            let mut variable = if value.contains(',') {
                value
            } else {
                value
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            if xml_name(script) == "msup" && !variable.ends_with(|c: char| c.is_ascii_digit()) {
                let prime = xml_nodes(script).last().ok_or_else(intent_error)?;
                if !prime_node(prime) {
                    return Err(intent_error());
                }
                variable.push_str(&text_content::decode(xml_first_text(prime))?);
            }
            (order.to_string(), variable)
        }
        _ => (String::new(), String::new()),
    };
    Ok(format!(":partial-derivative({order},$f,{variable})"))
}
fn formula_f_arg(nodes: &mut Vec<Xml>, index: usize) {
    let mut args = vec![];
    while index < nodes.len() {
        match xml_name(&nodes[index]) {
            "mi" => args.push(nodes.remove(index)),
            "mrow" => {
                args.push(nodes.remove(index));
                break;
            }
            "mn" => {
                if args.is_empty() {
                    args.push(nodes.remove(index));
                }
                break;
            }
            _ => break,
        }
    }
    let arg = if args.len() == 1 {
        let mut node = args.pop().unwrap();
        xml_attribute(&mut node, "arg", "f".into());
        node
    } else {
        attrs("mrow", vec![("arg".into(), "f".into())], args)
    };
    nodes.insert(index, arg);
}
// Formula#intent_post_processing runs after all children, including for nested
// Formula nodes. It deliberately preserves source errors and unusual mutations.
fn formula_postprocess(nodes: &mut Vec<Xml>) -> Result<(), String> {
    if nodes
        .first()
        .is_some_and(|node| xml_first_text(node) == "&#x2145;")
    {
        let second = nodes.get(1).ok_or_else(intent_error)?;
        if xml_name(second) == "mi" {
            let mut args = vec![];
            let mut name = String::new();
            let mut variable = String::new();
            loop {
                let next = nodes.get(1).ok_or_else(intent_error)?;
                match xml_name(next) {
                    "mi" => args.push(nodes.remove(1)),
                    "mrow" => {
                        for arg in &args {
                            name.push_str(&text_content::decode(xml_first_text(arg))?);
                        }
                        let children = xml_nodes(next);
                        variable = upcase_variable(
                            children
                                .get(1..children.len().saturating_sub(1))
                                .ok_or_else(intent_error)?,
                        )?;
                        args.push(nodes.remove(1));
                        break;
                    }
                    _ => break,
                }
            }
            nodes.insert(
                0,
                attrs(
                    "mrow",
                    vec![("intent".into(), format!(":derivative(1,{name},{variable})"))],
                    args,
                ),
            );
        }
    }
    let has_subsup = nodes.iter().enumerate().any(|(index, node)| {
        matches!(xml_name(node), "msub" | "msup" | "msubsup")
            && nodes
                .get(index + 1)
                .is_some_and(|next| xml_name(next) != "mo")
            && xml_nodes(node)
                .first()
                .is_some_and(|base| derivative_constant(xml_first_text(base)))
    });
    if has_subsup {
        let first = nodes
            .first()
            .and_then(|node| xml_nodes(node).first())
            .ok_or_else(intent_error)?;
        if !matches!(first, Xml::El(..)) {
            return Err(intent_error());
        }
        if derivative_constant(xml_first_text(first)) {
            let mut index = 0;
            while index < nodes.len() {
                let Some(base) = xml_nodes(&nodes[index]).first() else {
                    index += 1;
                    continue;
                };
                if !matches!(base, Xml::El(..)) || !derivative_constant(xml_first_text(base)) {
                    index += 1;
                    continue;
                }
                let node_type = xml_name(&nodes[index]).to_owned();
                let script = xml_nodes(&nodes[index]).get(1).ok_or_else(intent_error)?;
                let suffix = match node_type.as_str() {
                    "msub" => {
                        let (order, variable) = match xml_name(script) {
                            "mi" => ("1".into(), text_content::decode(xml_first_text(script))?),
                            "mn" => numeric_intent(script),
                            "mrow"
                                if xml_attr(script, "intent") == Some("fenced")
                                    || !xml_nodes(script).iter().all(|n| xml_name(n) == "mi") =>
                            {
                                ("1".into(), String::new())
                            }
                            _ => (String::new(), String::new()),
                        };
                        format!("({order},$f,{variable})")
                    }
                    "msup" => {
                        let order = match xml_name(script) {
                            "mi" => return Err(intent_error()),
                            "mn" => xml_first_text(script).into(),
                            "mrow" => "$n".into(),
                            _ => String::new(),
                        };
                        format!(
                            "({order},$f,{})",
                            text_content::decode(&superscript_variable(&nodes[index + 1..])?)?
                        )
                    }
                    _ => String::new(),
                };
                xml_attribute(&mut nodes[index], "intent", format!(":derivative{suffix}"));
                index += 1;
                let next = nodes.get(index).ok_or_else(intent_error)?;
                if matches!(xml_name(next), "mi" | "mrow") {
                    let after = nodes.get(index + 1).ok_or_else(intent_error)?;
                    if !matches!(xml_name(after), "mi" | "mrow") {
                        xml_attribute(&mut nodes[index], "arg", "f".into());
                    } else {
                        let terminal = nodes[index..].iter().find(|node| xml_name(node) != "mi");
                        if terminal.is_none_or(|node| xml_name(node) != "mrow") {
                            return Err(intent_error());
                        }
                    }
                    // Source wrap_in_mrow receives an Array slice, and its
                    // newly created wrapper is not inserted into this formula.
                }
                index += 1;
            }
        }
    }
    let partial = nodes.windows(2).any(|pair| {
        xml_name(&pair[0]) == "msub"
            && xml_nodes(&pair[0])
                .first()
                .is_some_and(|base| xml_first_text(base) == "&#x2202;")
    });
    if partial {
        let mut index = 0;
        while index < nodes.len() {
            if xml_name(&nodes[index]) == "msub"
                && xml_nodes(&nodes[index])
                    .first()
                    .is_some_and(|base| xml_first_text(base) == "&#x2202;")
            {
                let intent = partial_subscript_intent(&nodes[index])?;
                xml_attribute(&mut nodes[index], "intent", intent);
                formula_f_arg(nodes, index + 1);
            }
            index += 1;
        }
    }
    Ok(())
}

fn source_formula(values: Vec<Value>) -> Value {
    crate::model::formula(values)
}
fn filter_source_values(values: Vec<Value>) -> Value {
    match values.len() {
        0 => Value::Null,
        1 => values.into_iter().next().unwrap(),
        _ => source_formula(values),
    }
}
fn linebreak_index(values: &[Value]) -> Option<usize> {
    values.iter().enumerate().find_map(|(index, value)| {
        (value["class"] == "Math::Function::Linebreak" && index + 1 < values.len()).then_some(index)
    })
}
fn take_direct_line(values: &mut Vec<Value>, index: usize) -> Vec<Value> {
    let mut tail = values.split_off(index + 1);
    let br = values.pop().unwrap();
    if present(&br["fields"]["parameter_one"]) {
        let param = br["fields"]["parameter_one"].clone();
        if br["fields"]["attributes"]["linebreakstyle"] == "after" {
            values.push(param);
        } else {
            tail.insert(0, param);
        }
    }
    tail
}
fn source_break_child(
    value: &mut Value,
    obj: &mut Vec<Value>,
    optional: bool,
) -> Result<(), String> {
    if value.is_null() && optional {
        return Ok(());
    }
    if !value.is_object() || value.get("class").is_none() {
        return Err("source_render_error: non-node cannot receive line_breaking".into());
    }
    source_line_breaking(value, obj)
}
fn new_source_node(class: &str, fields: Value) -> Value {
    crate::model::node(class, fields)
}
fn formula_line_breaking(values: &mut Vec<Value>, obj: &mut Vec<Value>) -> Result<(), String> {
    if let Some(index) = linebreak_index(values) {
        *obj = take_direct_line(values, index);
        let mut nested = vec![];
        formula_line_breaking(values, &mut nested)?;
        if !nested.is_empty() {
            obj.insert(
                0,
                new_source_node(
                    "Math::Function::Linebreak",
                    serde_json::json!({"parameter_one":null,"attributes":{}}),
                ),
            );
            obj.insert(0, source_formula(nested));
        }
        return Ok(());
    }
    for index in 0..values.len() {
        source_break_child(&mut values[index], obj, false)?;
        if !obj.is_empty() {
            obj.extend(values.split_off(index + 1));
            break;
        }
    }
    Ok(())
}
// Constructor variable order and accessor/ancestry facts measured on source
// 0a1df68. The exporter and full observations accompany the task evidence.
fn source_class_properties(m: &Model) -> (&'static [&'static str], bool, bool) {
    match m.class.as_str() {
        "Math::Function::Multiscript" => (&["parameter_one", "parameter_two", "parameter_three"], true, false),
        "Math::Formula" => (
            &["value", "left_right_wrapper", "displaystyle"],
            false,
            false,
        ),
        "Math::Function::Abs"
        | "Math::Function::Arccos"
        | "Math::Function::Arcsin"
        | "Math::Function::Arctan"
        | "Math::Function::Cancel"
        | "Math::Function::Ceil"
        | "Math::Function::Cos"
        | "Math::Function::Cosh"
        | "Math::Function::Cot"
        | "Math::Function::Coth"
        | "Math::Function::Csc"
        | "Math::Function::Csch"
        | "Math::Function::Deg"
        | "Math::Function::Det"
        | "Math::Function::Dim"
        | "Math::Function::Exp"
        | "Math::Function::Floor"
        | "Math::Function::Gcd"
        | "Math::Function::Glb"
        | "Math::Function::Hom"
        | "Math::Function::Ker"
        | "Math::Function::Lcm"
        | "Math::Function::Left"
        | "Math::Function::Lg"
        | "Math::Function::Liminf"
        | "Math::Function::Limsup"
        | "Math::Function::Ln"
        | "Math::Function::Longdiv"
        | "Math::Function::Lub"
        | "Math::Function::Max"
        | "Math::Function::Mbox"
        | "Math::Function::Merror"
        | "Math::Function::Mglyph"
        | "Math::Function::Min"
        | "Math::Function::Mpadded"
        | "Math::Function::Ms"
        | "Math::Function::Msgroup"
        | "Math::Function::Msline"
        | "Math::Function::Norm"
        | "Math::Function::Phantom"
        | "Math::Function::Right"
        | "Math::Function::Scarries"
        | "Math::Function::Sec"
        | "Math::Function::Sech"
        | "Math::Function::Sin"
        | "Math::Function::Sinh"
        | "Math::Function::Sqrt"
        | "Math::Function::Substack"
        | "Math::Function::Sup"
        | "Math::Function::Tan"
        | "Math::Function::Tanh"
        | "Math::Function::Tr" => (&["parameter_one"], true, true),
        "Math::Function::Arg"
        | "Math::Function::Base"
        | "Math::Function::Color"
        | "Math::Function::FontStyle"
        | "Math::Function::FontStyle::Italic"
        | "Math::Function::FontStyle::Monospace"
        | "Math::Function::Frac"
        | "Math::Function::Inf"
        | "Math::Function::Intent"
        | "Math::Function::Lim"
        | "Math::Function::Log"
        | "Math::Function::Mod"
        | "Math::Function::Root"
        | "Math::Function::Over"
        | "Math::Function::Overset"
        | "Math::Function::Power"
        | "Math::Function::Stackrel"
        | "Math::Function::Td" => (&["parameter_one", "parameter_two"], true, false),
        "Math::Function::Bar"
        | "Math::Function::Ddot"
        | "Math::Function::Dot"
        | "Math::Function::Hat"
        | "Math::Function::Linebreak"
        | "Math::Function::Obrace"
        | "Math::Function::Tilde"
        | "Math::Function::Ubrace"
        | "Math::Function::Ul"
        | "Math::Function::Vec" => (&["parameter_one", "attributes"], true, true),
        "Math::Function::Fenced" => (
            &[
                "parameter_one",
                "parameter_two",
                "parameter_three",
                "options",
            ],
            true,
            false,
        ),
        "Math::Function::Int"
        | "Math::Function::Limits"
        | "Math::Function::Oint"
        | "Math::Function::PowerBase"
        | "Math::Function::Prod"
        | "Math::Function::Rule"
        | "Math::Function::Sum"
        | "Math::Function::Underover" => (
            &["parameter_one", "parameter_two", "parameter_three"],
            true,
            false,
        ),
        "Math::Function::Nary" => (
            &[
                "parameter_one",
                "parameter_two",
                "parameter_three",
                "parameter_four",
                "options",
            ],
            false,
            false,
        ),
        "Math::Function::Table"
        | "Math::Function::Table::Array"
        | "Math::Function::Table::Bmatrix"
        | "Math::Function::Table::Cases"
        | "Math::Function::Table::Eqarray"
        | "Math::Function::Table::Matrix"
        | "Math::Function::Table::Pmatrix"
        | "Math::Function::Table::Vmatrix" => (
            &["value", "open_paren", "close_paren", "options"],
            false,
            false,
        ),
        "Math::Function::Text" => (&["parameter_one", "lang"], true, true),
        "Math::Function::Underset" => (&["parameter_one", "parameter_two", "options"], true, false),
        "Math::Number" => (
            &["value", "mini_sub_sized", "mini_sup_sized", "base"],
            false,
            false,
        ),
        "Math::Symbols::Ampersand"
        | "Math::Symbols::Bigwedge"
        | "Math::Symbols::Cap"
        | "Math::Symbols::Cdots"
        | "Math::Symbols::Clockoint"
        | "Math::Symbols::Cntclockoint"
        | "Math::Symbols::Comma"
        | "Math::Symbols::Coprod"
        | "Math::Symbols::Dd"
        | "Math::Symbols::Ddots"
        | "Math::Symbols::Del"
        | "Math::Symbols::Dint"
        | "Math::Symbols::Dot"
        | "Math::Symbols::Duni"
        | "Math::Symbols::Equal"
        | "Math::Symbols::Ii"
        | "Math::Symbols::Iiiint"
        | "Math::Symbols::Iiint"
        | "Math::Symbols::Iint"
        | "Math::Symbols::Int"
        | "Math::Symbols::Intclockwise"
        | "Math::Symbols::Intercal"
        | "Math::Symbols::Jj"
        | "Math::Symbols::Minus"
        | "Math::Symbols::Oiiint"
        | "Math::Symbols::Oiint"
        | "Math::Symbols::Oint"
        | "Math::Symbols::Paren::Langle"
        | "Math::Symbols::Paren::Lceil"
        | "Math::Symbols::Paren::Lcurly"
        | "Math::Symbols::Paren::Lfloor"
        | "Math::Symbols::Paren::Lround"
        | "Math::Symbols::Paren::Lsquare"
        | "Math::Symbols::Paren::Norm"
        | "Math::Symbols::Paren::Rangle"
        | "Math::Symbols::Paren::Rceil"
        | "Math::Symbols::Paren::Rcurly"
        | "Math::Symbols::Paren::Rfloor"
        | "Math::Symbols::Paren::Rround"
        | "Math::Symbols::Paren::Rsquare"
        | "Math::Symbols::Paren::Vert"
        | "Math::Symbols::Planckconst"
        | "Math::Symbols::Plus"
        | "Math::Symbols::Pm"
        | "Math::Symbols::Prime"
        | "Math::Symbols::Rightarrow"
        | "Math::Symbols::Space"
        | "Math::Symbols::Sum"
        | "Math::Symbols::Symbol"
        | "Math::Symbols::Theta"
        | "Math::Symbols::UpcaseDd"
        | "Math::Symbols::Vdots"
        | "Math::Symbols::Vvv" => (&["value"], false, false),
        _ => (&[], false, false),
    }
}
fn core_variable_keys(m: &Model) -> Vec<String> {
    let ordered = source_class_properties(m).0;
    let mut keys: Vec<String> = ordered
        .iter()
        .filter(|key| m.fields.contains_key(**key))
        .map(|key| (*key).to_owned())
        .collect();
    for key in m.fields.keys() {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    keys
}
fn source_has_hide(m: &Model) -> bool {
    source_class_properties(m).1
}
fn core_updated_values(
    node: &mut Value,
    key: &str,
    obj: &mut Vec<Value>,
    array_alias: bool,
) -> Result<(), String> {
    let m = Model::from_value(node)?;
    let mut next = node.clone();
    let mut found = false;
    for variable in core_variable_keys(&m) {
        if variable == key {
            next["fields"][&variable] = filter_source_values(std::mem::take(obj));
            found = true;
        } else if found {
            node["fields"][&variable] = Value::Null;
        }
    }
    if source_has_hide(&m) {
        next["fields"]["hide_function_name"] = Value::Bool(true);
    }
    *obj = vec![next];
    if array_alias {
        // Core#updated_object_values stores the *same* Formula object in the
        // original slot, then obj.update replaces that object's value. Capture
        // its resulting state explicitly rather than introducing shared state.
        node["fields"][key] = source_formula(obj.clone());
    }
    Ok(())
}
fn source_line_breaking(node: &mut Value, obj: &mut Vec<Value>) -> Result<(), String> {
    if node["class"] == "Math::Function::Uncertainty" {return Ok(());}
    if node["class"]=="Math::Function::Td" {return table_xml::td_line_breaking(node);}
    if node["class"]=="Math::Function::Stackrel" {
        source_line_breaking(&mut node["fields"]["parameter_one"],obj)?;
        if !obj.is_empty(){*obj=vec![new_source_node("Math::Function::Stackrel",serde_json::json!({"parameter_one":filter_source_values(std::mem::take(obj)),"parameter_two":node["fields"]["parameter_two"]}))];node["fields"]["parameter_two"]=Value::Null;}
        return Ok(());
    }

    if node["class"]=="Math::Function::Log" {return mod_log::log_line_breaking(node,obj);}
    if node["class"]=="Math::Function::Underset" {return final_options::underset_line_breaking(node,obj); }
    if node["class"] == "Math::Function::Fenced" { return fences::line_breaking(node, obj); }
    let m = Model::from_value(node)
        .map_err(|_| "source_render_error: non-node cannot receive line_breaking")?;
    let kind = m.kind();
    if line_break_overrides::handles(&m) { return line_break_overrides::line_breaking(node, obj); }
    if nary::handles(&m) { return nary::line_breaking(node, obj); }
    if m.class == "Math::Function::Nary" { return generic_nary::line_breaking(node, obj); }
    if kind == "Multiscript" {
        source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
        if !obj.is_empty() {
            *obj = vec![new_source_node(&m.class, serde_json::json!({
                "parameter_one": filter_source_values(std::mem::take(obj)),
                "parameter_two": node["fields"]["parameter_two"],
                "parameter_three": node["fields"]["parameter_three"]
            }))];
            node["fields"]["parameter_two"] = Value::Null;
            node["fields"]["parameter_three"] = Value::Null;
            return Ok(());
        }
        let field = node["fields"]["parameter_two"].clone();
        let values = field.as_array().ok_or("source_render_error: multiscript subscript field has no array line-breaking methods")?;
        if linebreak_index(values).is_some() {
            let mut values = values.clone();
            formula_line_breaking(&mut values, obj)?;
            core_updated_values(node, "parameter_two", obj, true)?;
        } else {
            for child in node["fields"]["parameter_two"].as_array_mut().unwrap() {
                source_break_child(child, obj, false)?;
            }
        }
        if !obj.is_empty() {
            *obj = vec![new_source_node(&m.class, serde_json::json!({
                "parameter_one": null,
                "parameter_two": std::mem::take(obj),
                "parameter_three": node["fields"]["parameter_three"]
            }))];
            node["fields"]["parameter_three"] = Value::Null;
        }
        return Ok(());
    }
    if matches!(kind, "Formula" | "Mrow" | "Mstyle") {
        let values = node["fields"]["value"]
            .as_array_mut()
            .ok_or("source_render_error: formula value must be an array")?;
        return formula_line_breaking(values, obj);
    }
    if kind == "Fenced" {
        let Some(values) = node["fields"]["parameter_two"].as_array_mut() else {
            return Ok(());
        };
        if let Some(index) = linebreak_index(values) {
            let tail = take_direct_line(values, index);
            let mut next = node.clone();
            next["fields"]["parameter_one"] = Value::Null;
            next["fields"]["parameter_two"] = Value::Array(tail);
            node["fields"]["parameter_three"] = Value::Null;
            *obj = vec![next];
        }
        return Ok(());
    }
    if matches!(m.class.as_str(),"Math::Function::Phantom"|"Math::Function::Msgroup"|"Math::Function::Longdiv") {
        return custom_array_line_breaking(node,obj);
    }
    let drop = matches!(
        kind,
        "Vec" | "Dot" | "Ddot" | "Tilde" | "Ubrace" | "Obrace" | "Bar" | "Sqrt"
    );
    let delimiter = matches!(kind, "Abs" | "Ceil" | "Floor" | "Norm");
    let font = m.class.starts_with("Math::Function::FontStyle::")
        || m.class == "Math::Function::FontStyle";
    if drop || delimiter || kind == "Hat" || font {
        source_break_child(&mut node["fields"]["parameter_one"], obj, !delimiter)?;
        if obj.is_empty() {
            return Ok(());
        }
        let value = filter_source_values(std::mem::take(obj));
        if drop {
            *obj = if value.is_null() { vec![] } else { vec![value] };
        } else if kind == "Hat" {
            *obj = vec![new_source_node(
                "Math::Function::Overset",
                serde_json::json!({"parameter_one":null,"parameter_two":value}),
            )];
        } else if delimiter {
            *obj = vec![new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":value,"open_paren":true,"close_paren":false}),
            )];
            node["fields"]["close_paren"] = Value::Bool(true);
            if !present(m.field("open_paren")) {
                node["fields"]["open_paren"] = Value::Bool(false);
            }
        } else {
            *obj = vec![new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":value,"parameter_two":m.field("parameter_two")}),
            )];
        }
        return Ok(());
    }
    if matches!(kind, "Lim" | "Inf") {
        source_break_child(&mut node["fields"]["parameter_one"], obj, false)?;
        if !obj.is_empty() {
            *obj = vec![new_source_node(
                "Math::Function::Underover",
                serde_json::json!({"parameter_one":null,"parameter_two":filter_source_values(std::mem::take(obj)),"parameter_three":node["fields"]["parameter_two"]}),
            )];
            node["fields"]["parameter_two"] = Value::Null;
        }
        return Ok(());
    }
    if matches!(kind, "Power" | "Base" | "Frac" | "Over") {
        source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
        if !obj.is_empty() {
            let mut next = new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":filter_source_values(std::mem::take(obj)),"parameter_two":node["fields"]["parameter_two"]}),
            );
            if kind == "Frac" {
                next["fields"]["hide_function_name"] = Value::Bool(true);
            }
            if kind == "Over" {
                node["fields"]["hide_function_name"] = Value::Bool(true);
            }
            node["fields"]["parameter_two"] = Value::Null;
            *obj = vec![next];
            return Ok(());
        }
        if kind == "Power" {
            return Ok(());
        }
        source_break_child(&mut node["fields"]["parameter_two"], obj, true)?;
        if !obj.is_empty() {
            let mut next = new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":null,"parameter_two":filter_source_values(std::mem::take(obj))}),
            );
            if matches!(kind, "Frac" | "Over") {
                next["fields"]["hide_function_name"] = Value::Bool(true);
            }
            *obj = vec![next];
        }
        return Ok(());
    }
    if matches!(kind, "Limits" | "Underover" | "PowerBase") {
        source_break_child(&mut node["fields"]["parameter_one"], obj, true)?;
        if !obj.is_empty() {
            *obj = vec![new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":filter_source_values(std::mem::take(obj)),"parameter_two":node["fields"]["parameter_two"],"parameter_three":node["fields"]["parameter_three"]}),
            )];
            node["fields"]["parameter_two"] = Value::Null;
            node["fields"]["parameter_three"] = Value::Null;
            return Ok(());
        }
        source_break_child(&mut node["fields"]["parameter_two"], obj, kind == "Limits")?;
        if !obj.is_empty() {
            *obj = vec![new_source_node(
                &m.class,
                serde_json::json!({"parameter_one":null,"parameter_two":filter_source_values(std::mem::take(obj)),"parameter_three":node["fields"]["parameter_three"]}),
            )];
            if kind == "Underover" {
                node["fields"]["parameter_two"] = Value::Null;
            }
            node["fields"]["parameter_three"] = Value::Null;
        }
        return Ok(());
    }
    // Core#line_breaking deliberately keeps iterating array children after a
    // break. Their methods see the same continuation Formula and can replace it.
    for key in core_variable_keys(&m) {
        let field = node["fields"][&key].clone();
        if field.is_object() && field.get("class").is_some() {
            source_line_breaking(&mut node["fields"][&key], obj)?;
            if !obj.is_empty() {
                core_updated_values(node, &key, obj, false)?;
            }
        } else if let Some(array) = field.as_array() {
            if linebreak_index(array).is_some() {
                let mut formula = source_formula(array.clone());
                source_line_breaking(&mut formula, obj)?;
                core_updated_values(node, &key, obj, true)?;
            } else {
                for index in 0..array.len() {
                    source_break_child(&mut node["fields"][&key][index], obj, false)?;
                }
            }
        }
    }
    Ok(())
}
fn split_node_once(node: &mut Value) -> Result<Vec<Value>, String> {
    let mut obj = vec![];
    source_line_breaking(node, &mut obj)?;
    Ok(obj)
}

fn split_formulas(m: &Model) -> Result<Vec<Model>, String> {
    let mut current = m.value();
    let mut lines = vec![];
    loop {
        let tail = split_node_once(&mut current)?;
        lines.push(Model::from_value(&current)?);
        if tail.is_empty() {
            break;
        }
        current = source_formula(tail);
    }
    Ok(lines)
}
fn source_formula_array(value: &Value) -> bool {
    matches!(
        value["class"].as_str(),
        Some("Math::Formula" | "Math::Formula::Mrow" | "Math::Formula::Mstyle")
    ) && !present(&value["fields"]["left_right_wrapper"])
}
fn root_xml(m: &Model, format: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    if !matches!(
        m.class.as_str(),
        "Math::Formula" | "Math::Formula::Mrow" | "Math::Formula::Mstyle"
    ) {
        return render_xml(m, format, ctx);
    }
    let mut content = vec![];
    for value in children(m)? {
        if format == "mathml" && ctx.intent && source_formula_array(&value) {
            return Err(intent_error());
        }
        content.extend(render_xml(&Model::from_value(&value)?, format, ctx)?);
    }
    if format == "mathml" && ctx.intent {
        formula_postprocess(&mut content)?;
    }
    Ok(content)
}
fn mathml_document(content: Vec<Xml>, display_style: bool) -> String {
    format!(
        "{}\n",
        dump(
            &attrs(
                "math",
                vec![
                    ("xmlns".into(), "http://www.w3.org/1998/Math/MathML".into()),
                    ("display".into(), "block".into()),
                ],
                vec![attrs(
                    "mstyle",
                    vec![("displaystyle".into(), display_style.to_string())],
                    content
                )]
            ),
            0
        )
    )
}

pub fn write(model: &Value, format: &str, options: &Value) -> Result<String, String> {
    let m = Model::from_value(model)?;
    let opts = options.as_object().ok_or("options must be an object")?;
    if format == "omml" && opts.contains_key("intent") {
        return Err("source_argument_error: unknown keyword: :intent".into());
    }
    for k in opts.keys() {
        if ![
            "displayStyle",
            "intent",
            "splitOnLinebreak",
            "unaryFunctionSpacing",
            "formatterOptions",
            "format",
        ]
        .contains(&k.as_str())
        {
            return Err(format!("unported promised writer option {k}"));
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
    if ["mathml","omml"].contains(&format) && opts.contains_key("format") { return Err("source_argument_error: unknown keyword: :options".into()); }
    let formatter_request = formatter_context::decode(opts.get("formatterOptions"), opts.get("format"))?;
    let ds = opts
        .get("displayStyle")
        .unwrap_or_else(|| m.field("displaystyle"));
    let formula_input=m.field("input_string").clone();
    let context = WriterContext {
        formatter: formatter_request.as_ref(),
        table:false,
        formula_input: if m.class=="Math::Formula"||m.class.starts_with("Math::Formula::") {Some(&formula_input)}else{None},
        source_format: format,
        display_style: ds == true || ds == "true",
        unary_function_spacing: opts
            .get("unaryFunctionSpacing")
            .map(present)
            .unwrap_or(true),
        intent: opts.get("intent").map(present).unwrap_or(false),
    };
    let ctx = &context;
    if !["mathml", "omml"].contains(&format) {
        if opts.keys().any(|k| !["formatterOptions","format"].contains(&k.as_str())) {
            return Err("writer options not yet ported for text formats".into());
        }
        return render_text(&m, format, ctx);
    }
    let formulas = if opts.get("splitOnLinebreak").map(present).unwrap_or(false) {
        uncertainty_writers::before_split(&m, format, ctx)?;
        split_formulas(&m).map_err(|error| {
            if format == "mathml" {
                if let Some(message) = error.strip_prefix("source_render_error: ") {
                    return format!("source_no_method_error: {message}");
                }
            }
            error
        })?
    } else {
        vec![m]
    };
    if format == "mathml" {
        let mut output = String::new();
        for formula in formulas {
            output.push_str(&mathml_document(
                root_xml(&formula, format, ctx)?,
                ctx.display_style,
            ));
        }
        Ok(output)
    } else {
        let ns: Vec<(String, String)> =
            serde_json::from_str(include_str!("omml-namespaces.json")).unwrap();
        let mut nodes = vec![];
        for (index, formula) in formulas.iter().enumerate() {
            if index > 0 {
                nodes.push(el("m:r", vec![el("br", vec![])]));
            }
            nodes.push(el("m:oMath", root_xml(formula, format, ctx)?));
        }
        Ok(format!("{}\n", dump(&attrs("m:oMathPara", ns, nodes), 0)))
    }
}

// Shared UnaryFunction#custom_array_line_breaking source procedure.
fn custom_array_line_breaking(node:&mut Value,obj:&mut Vec<Value>)->Result<(),String>{
    let m=Model::from_value(node)?;
    let values = node["fields"]["parameter_one"]
        .as_array_mut()
        .ok_or("source_render_error: phantom line breaking requires an array")?;
    if let Some(index) = linebreak_index(values) {
        let tail = take_direct_line(values, index);
        *obj = vec![new_source_node(
            &m.class,
            serde_json::json!({"parameter_one":tail}),
        )];
        let mut nested = vec![];
        // UnaryFunction#reprocess_parameter_one calls the same custom
        // algorithm on the first line before prepending a pending line.
        source_line_breaking(node, &mut nested)?;
        if !nested.is_empty() {
            obj.insert(
                0,
                new_source_node(
                    "Math::Function::Linebreak",
                    serde_json::json!({"parameter_one":null,"attributes":{}}),
                ),
            );
            obj.insert(
                0,
                new_source_node(&m.class, serde_json::json!({"parameter_one":nested})),
            );
        }
    } else {
        for index in 0..values.len() {
            source_break_child(&mut values[index], obj, false)?;
            if !obj.is_empty() {
                obj.extend(values.split_off(index + 1));
                break;
            }
        }
    }
    Ok(())
}
