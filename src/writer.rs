//! Procedural source-writer port; unported nodes/options fail, never return approximations.
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
    let mut x = s.replace('<', "&lt;").replace('>', "&gt;");
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
                "{pad}<{n}{at}>\n{}\n{pad}</{n}>",
                c.iter()
                    .map(|v| dump(v, depth + 1))
                    .collect::<Vec<_>>()
                    .join("\n")
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
        return value_text(m.field("value"));
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
fn fence_text(m: &Model, f: &str) -> Result<String, String> {
    let options = m.field("options");
    if !options.is_null() && options.as_object().map_or(true, |o| !o.is_empty()) {
        return Err("unported promised fence options".into());
    }
    let a = child(m.field("parameter_one"), f)?;
    let b = child(m.field("parameter_three"), f)?;
    let body = m
        .field("parameter_two")
        .as_array()
        .ok_or("fence.parameter_two must be node array")?
        .iter()
        .map(|v| child(v, f))
        .collect::<Result<Vec<_>, _>>()?
        .join(if f == "html" { "" } else { " " });
    Ok(match f {
        "latex" => format!("{a} {body} {b}"),
        "html" => format!("<i>{a}</i>{body}<i>{b}</i>"),
        _ => format!("{a}{body}{b}"),
    })
}
fn render_text(m: &Model, f: &str) -> Result<String, String> {
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
            Ok(c.join(if f == "unicodemath" { " " } else { " " }))
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
                "asciimath" => format!("frac({a})({b})"),
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
        _ => Err(format!("unported promised writer node {}", m.class)),
    }
}
fn render_xml(m: &Model, f: &str) -> Result<Vec<Xml>, String> {
    if m.class.starts_with("Math::Symbols::") {
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
            let a = child(m.field("parameter_one"), "asciimath")?;
            let b = child(m.field("parameter_three"), "asciimath")?;
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
                body.extend(render_xml(&Model::from_value(v)?, f)?)
            }
            if f == "mathml" {
                let mut c = vec![el("mo", vec![text(&a)])];
                c.extend(body);
                c.push(el("mo", vec![text(&b)]));
                Ok(vec![el("mrow", c)])
            } else {
                let props = el(
                    "m:dPr",
                    vec![
                        attrs("m:begChr", vec![("m:val".into(), a)], vec![]),
                        attrs("m:sepChr", vec![("m:val".into(), "".into())], vec![]),
                        attrs("m:endChr", vec![("m:val".into(), b)], vec![]),
                    ],
                );
                Ok(vec![el("m:d", vec![props, el("m:e", body)])])
            }
        }
        "Formula" | "Mrow" | "Mstyle" => {
            let mut c = vec![];
            for v in children(m)? {
                c.extend(render_xml(&Model::from_value(&v)?, f)?)
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
                if v.is_null() {
                    Ok(vec![])
                } else {
                    render_xml(&Model::from_value(v)?, f)
                }
            };
            let a = slot("parameter_one")?;
            let b = slot("parameter_two")?;
            if f == "mathml" {
                let mut c = a;
                c.extend(b);
                Ok(vec![el("mfrac", c)])
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
        _ => Err(format!("unported promised writer node {}", m.class)),
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
            if k == "intent" && v == false {
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
    let c = if m.kind() == "Formula" {
        let mut c = vec![];
        for v in children(&m)? {
            c.extend(render_xml(&Model::from_value(&v)?, format)?)
        }
        c
    } else {
        render_xml(&m, format)?
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
