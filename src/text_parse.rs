//! Accepted LaTeX/Unicode stage captures -> existing source common-node wire.
//! Parsing is delegated to the pinned stage crates; this module only transforms captures.
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
thread_local! {
    static LATEX: RefCell<Option<latexmath::Parser>> = const { RefCell::new(None) };
    static UNICODE: RefCell<Option<umath::Parser>> = const { RefCell::new(None) };
}
fn error(code: &str, message: impl ToString) -> Value {
    json!({"code":code,"message":message.to_string()})
}
fn incomplete(message: impl ToString) -> Value {
    error("INCOMPLETE_TEXT_TRANSLATION", message)
}
fn untag(v: &Value) -> Result<Value, Value> {
    if let Some(s) = v.get("slice") {
        return Ok(s.clone());
    }
    if let Some(a) = v.get("array").and_then(Value::as_array) {
        return a
            .iter()
            .map(untag)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    if let Some(a) = v.get("hash").and_then(Value::as_array) {
        let mut m = Map::new();
        for pair in a {
            let key = pair[0]
                .get("sym")
                .or_else(|| pair[0].get("str"))
                .and_then(Value::as_str)
                .ok_or_else(|| incomplete("invalid canonical capture key"))?;
            if m.insert(key.into(), untag(&pair[1])?).is_some() {
                return Err(incomplete("duplicate canonical capture key"));
            }
        }
        return Ok(Value::Object(m));
    }
    if let Some(a) = v.as_array() {
        return a
            .iter()
            .map(untag)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    Ok(v.clone())
}
fn one(mut values: Vec<Value>) -> Value {
    if values.len() == 1 {
        values.pop().unwrap()
    } else {
        formula(values)
    }
}
fn unfence(v: Value) -> Value {
    if v["class"] == "Math::Function::Fenced" {
        if let Some(a) = v["fields"]["parameter_two"].as_array() {
            return one(a.clone());
        }
    }
    v
}
fn fenced(left: Value, body: Vec<Value>, right: Value) -> Value {
    node(
        "Math::Function::Fenced",
        json!({"options":{},"parameter_one":left,"parameter_two":body,"parameter_three":right}),
    )
}
fn unary(name: &str, arg: Value) -> Result<Value, Value> {
    let class = match name {
        "sqrt" => "Math::Function::Sqrt",
        "hat" => "Math::Function::Hat",
        "vec" => "Math::Function::Vec",
        "bar" => "Math::Function::Bar",
        "dot" => "Math::Function::Dot",
        "ddot" => "Math::Function::Ddot",
        "tilde" => "Math::Function::Tilde",
        "sin" => "Math::Function::Sin",
        "cos" => "Math::Function::Cos",
        "tan" => "Math::Function::Tan",
        "ln" => "Math::Function::Ln",

        _ => {
            return Err(incomplete(format!(
                "unported source unary constructor {name}"
            )))
        }
    };
    let mut fields = json!({"parameter_one":arg});
    if ["hat", "vec", "bar", "dot", "ddot", "tilde"].contains(&name) {
        fields["attributes"] = json!({})
    }
    Ok(node(class, fields))
}
fn tx(format: &str, v: &Value, depth: usize) -> Result<Vec<Value>, Value> {
    if depth > 128 {
        return Err(error("RESOURCE_LIMIT", "common converter depth exceeds128"));
    }
    if let Some(a) = v.as_array() {
        let mut out = Vec::new();
        for x in a {
            out.extend(tx(format, x, depth + 1)?)
        }
        return Ok(out);
    }
    if v.is_null() {
        return Ok(Vec::new());
    }
    let m = v
        .as_object()
        .ok_or_else(|| incomplete("capture scalar outside a source leaf rule"))?;
    // Parent script productions retain operand order explicitly, never hash sort order.
    if m.contains_key("base") && (m.contains_key("sub") || m.contains_key("sup")) {
        if m.keys()
            .any(|key| !["base", "sub", "sup"].contains(&key.as_str()))
        {
            return Err(incomplete("unported additional source capture fields"));
        }
        let base = one(tx(format, &m["base"], depth + 1)?);
        return if m.contains_key("sub") && m.contains_key("sup") {
            Ok(vec![node(
                "Math::Function::PowerBase",
                json!({"parameter_one":base,"parameter_two":unfence(one(tx(format, &m["sub"],depth+1)?)),"parameter_three":unfence(one(tx(format, &m["sup"],depth+1)?))}),
            )])
        } else {
            let (key, class) = if m.contains_key("sub") {
                ("sub", "Math::Function::Base")
            } else {
                ("sup", "Math::Function::Power")
            };
            Ok(vec![node(
                class,
                json!({"parameter_one":base,"parameter_two":unfence(one(tx(format, &m[key],depth+1)?))}),
            )])
        };
    }
    if m.contains_key("supscript") || m.contains_key("subscript") {
        let mut rest = m.clone();
        let sub = rest.remove("subscript");
        let sup = rest.remove("supscript");
        let base = one(tx(format, &Value::Object(rest), depth + 1)?);
        return match (sub, sup) {
            (Some(a), Some(b)) => Ok(vec![node(
                "Math::Function::PowerBase",
                json!({"parameter_one":base,"parameter_two":one(tx(format, &a,depth+1)?),"parameter_three":one(tx(format, &b,depth+1)?)}),
            )]),
            (Some(a), None) => Ok(vec![node(
                "Math::Function::Base",
                json!({"parameter_one":base,"parameter_two":one(tx(format, &a,depth+1)?)}),
            )]),
            (None, Some(a)) => Ok(vec![node(
                "Math::Function::Power",
                json!({"parameter_one":base,"parameter_two":one(tx(format, &a,depth+1)?)}),
            )]),
            _ => unreachable!(),
        };
    }
    if m.len() == 2 && m.contains_key("fonts") && m.contains_key("intermediate_exp") {
        let font = m["fonts"]
            .as_str()
            .ok_or_else(|| incomplete("non-text font selector"))?;
        let class = match font {
            "mathbb" => "DoubleStruck",
            "mathbf" => "Bold",
            "textrm" | "mathrm" => "Normal",
            "mathcal" => "Script",
            "mathfrak" => "Fraktur",
            "mathsf" => "SansSerif",
            "mathtt" => "Monospace",
            "mathit" => "Italic",
            _ => {
                return Err(incomplete(format!(
                    "unported source font constructor {font}"
                )))
            }
        };
        return Ok(vec![node(
            &format!("Math::Function::FontStyle::{class}"),
            json!({"parameter_one":one(tx(format, &m["intermediate_exp"],depth+1)?),"parameter_two":font}),
        )]);
    }
    if m.contains_key("numerator") && m.contains_key("denominator") {
        if m.keys()
            .any(|key| !["numerator", "denominator"].contains(&key.as_str()))
        {
            return Err(incomplete("unported additional source capture fields"));
        }
        return Ok(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":unfence(one(tx(format, &m["numerator"],depth+1)?)),"parameter_two":unfence(one(tx(format, &m["denominator"],depth+1)?))}),
        )]);
    }
    if let Some(name) = m.get("binary").and_then(Value::as_str) {
        if m.keys()
            .any(|key| !["binary", "first_value", "second_value"].contains(&key.as_str()))
        {
            return Err(incomplete("unported additional source capture fields"));
        }
        if name != "frac" {
            return Err(incomplete(format!(
                "unported source binary constructor {name}"
            )));
        }
        return Ok(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":one(tx(format, m.get("first_value").ok_or_else(||incomplete("missing fraction numerator"))?,depth+1)?),"parameter_two":one(tx(format, m.get("second_value").ok_or_else(||incomplete("missing fraction denominator"))?,depth+1)?)}),
        )]);
    }
    if m.contains_key("sqrt") {
        if m.keys()
            .any(|key| !["sqrt", "intermediate_exp"].contains(&key.as_str()))
        {
            return Err(incomplete("unported additional source capture fields"));
        }
        return Ok(vec![unary(
            "sqrt",
            one(tx(
                format,
                m.get("intermediate_exp")
                    .ok_or_else(|| incomplete("missing sqrt operand"))?,
                depth + 1,
            )?),
        )?]);
    }
    if m.contains_key("root_symbol") {
        if m.keys()
            .any(|key| !["root_symbol", "first_value"].contains(&key.as_str()))
        {
            return Err(incomplete("unported additional source capture fields"));
        }
        if m["root_symbol"] != "&#x221a;" && m["root_symbol"] != "√" {
            return Err(incomplete("unported indexed Unicode radical"));
        }
        return Ok(vec![unary(
            "sqrt",
            one(tx(
                format,
                m.get("first_value")
                    .ok_or_else(|| incomplete("missing radical operand"))?,
                depth + 1,
            )?),
        )?]);
    }
    if m.get("unary").is_some_and(Value::is_string)
        || m.get("unary_functions").is_some_and(Value::is_string)
    {
        if m.len() != 2 || !m.contains_key("first_value") {
            return Err(incomplete("unported additional unary capture fields"));
        }
        let name = m
            .get("unary")
            .or_else(|| m.get("unary_functions"))
            .and_then(Value::as_str)
            .ok_or_else(|| incomplete("non-text unary selector"))?;
        return Ok(vec![unary(
            name,
            one(tx(
                format,
                m.get("first_value")
                    .ok_or_else(|| incomplete("missing unary operand"))?,
                depth + 1,
            )?),
        )?]);
    }
    if m.len() == 2 && m.contains_key("text") && m.contains_key("first_value") {
        let name = m["text"]
            .as_str()
            .ok_or_else(|| incomplete("non-text text command"))?;
        let body = if let Some(text) = m["first_value"].as_str() {
            text.to_string()
        } else if let Some(parts) = m["first_value"].as_array() {
            parts
                .iter()
                .map(|x| x.as_str().ok_or_else(|| incomplete("non-string text part")))
                .collect::<Result<Vec<_>, _>>()?
                .join("")
        } else {
            return Err(incomplete("unported text command payload"));
        };
        return match name {
            "text" => Ok(vec![node(
                "Math::Function::Text",
                json!({"lang":null,"parameter_one":body}),
            )]),
            "mbox" => Ok(vec![node(
                "Math::Function::Mbox",
                json!({"parameter_one":body}),
            )]),
            _ => Err(incomplete(format!(
                "unported source text constructor {name}"
            ))),
        };
    }
    if m.contains_key("left") && m.contains_key("right") {
        if m.len() != 5
            || !m.contains_key("left_paren")
            || !m.contains_key("right_paren")
            || !m.contains_key("expression")
        {
            return Err(incomplete("unported left/right capture fields"));
        }
        let delimiter = |value: &Value| -> Result<Value, Value> {
            let text = value
                .as_str()
                .ok_or_else(|| incomplete("non-string left/right delimiter"))?;
            let mapped = match text {
                "\\backslash" => Some("&#x5c;"),
                "\\langle" => Some("&#x2329;"),
                "\\rangle" => Some("&#x232a;"),
                "\\lfloor" => Some("&#x230a;"),
                "\\rfloor" => Some("&#x230b;"),
                "\\lceil" => Some("&#x2308;"),
                "\\rceil" => Some("&#x2309;"),
                "\\lbrace" => Some("&#x7b;"),
                "\\rbrace" => Some("&#x7d;"),
                "\\lbrack" => Some("&#x5b;"),
                "\\rbrack" => Some("&#x5d;"),
                "\\Vert" | "\\|" => Some("&#x2016;"),
                "\\vert" => Some("&#x7c;"),
                "\\{" => Some("{"),
                "\\}" => Some("}"),
                "(" | ")" | "<" | ">" | "/" | "|" | "[" | "]" => Some(text),
                _ => None,
            };
            Ok(mapped.map_or(Value::Null, |x| json!(x)))
        };
        let body = one(tx(format, &m["expression"], depth + 1)?);
        let mut result = formula(vec![
            node(
                "Math::Function::Left",
                json!({"parameter_one":delimiter(&m["left_paren"])?}),
            ),
            body,
            node(
                "Math::Function::Right",
                json!({"parameter_one":delimiter(&m["right_paren"])?}),
            ),
        ]);
        result["fields"]["left_right_wrapper"] = json!(false);
        return Ok(vec![result]);
    }
    let open = m
        .get("left_paren")
        .or_else(|| m.get("open_paren"))
        .or_else(|| m.get("opener"));
    let close = m
        .get("right_paren")
        .or_else(|| m.get("close_paren"))
        .or_else(|| m.get("closer"));
    if let (Some(l), Some(r)) = (open, close) {
        let left = if let Some(s) = l.as_str() {
            crate::symbols::symbol_for(s, format)
        } else {
            one(tx(format, l, depth + 1)?)
        };
        let right = if let Some(s) = r.as_str() {
            crate::symbols::symbol_for(s, format)
        } else {
            one(tx(format, r, depth + 1)?)
        };
        let mut rest = m.clone();
        for key in [
            "left_paren",
            "right_paren",
            "open_paren",
            "close_paren",
            "opener",
            "closer",
        ] {
            rest.remove(key);
        }
        return Ok(vec![fenced(
            left,
            tx(format, &Value::Object(rest), depth + 1)?,
            right,
        )]);
    }
    if m.len() == 1 {
        let (key, x) = m.iter().next().unwrap();
        if key == "number" {
            return x
                .as_str()
                .map(|s| vec![number(s)])
                .ok_or_else(|| incomplete("non-text number capture"));
        }
        if [
            "symbol",
            "symbols",
            "ordinary_symbols",
            "alphanumeric",
            "operant",
            "operator",
            "lparen",
            "rparen",
            "open_paren",
            "close_paren",
            "unicode_symbols",
            "binary_symbols",
            "relational_symbols",
        ]
        .contains(&key.as_str())
        {
            return x
                .as_str()
                .map(|s| vec![crate::symbols::symbol_for(s, format)])
                .ok_or_else(|| incomplete("non-text symbol capture"));
        }
        if key == "text" || key == "space" {
            let s = if key == "space" {
                " "
            } else {
                x.as_str()
                    .ok_or_else(|| incomplete("non-text text capture"))?
            };
            return Ok(vec![node(
                "Math::Function::Text",
                json!({"lang":null,"parameter_one":s}),
            )]);
        }
        if ["hex_number", "binary_number", "octal_number"].contains(&key.as_str()) {
            let base = if key == "hex_number" {
                16
            } else if key == "binary_number" {
                2
            } else {
                8
            };
            let mut value = number(
                x.as_str()
                    .ok_or_else(|| incomplete("non-text based number"))?,
            );
            value["fields"]["base"] = json!(base);
            return Ok(vec![value]);
        }
        if [
            "expression",
            "expr",
            "sequence",
            "power",
            "binary",
            "unary",
            "unary_functions",
            "intermediate_exp",
            "factor",
            "digit",
            "atom",
            "frac",
            "root",
            "sup_exp",
            "sub_exp",
            "subsup_exp",
            "power_base",
            "base",
            "left_right",
            "under_over",
            "ternary_class",
            "ternary",
            "unary_subsup",
            "sup_script",
            "sub_script",
            "int_exp",
            "operand",
            "first_value",
        ]
        .contains(&key.as_str())
        {
            return tx(format, x, depth + 1);
        }
    }
    if m.contains_key("sequence") && m.contains_key("expression") && m.len() == 2 {
        let mut out = tx(format, &m["sequence"], depth + 1)?;
        out.extend(tx(format, &m["expression"], depth + 1)?);
        return Ok(out);
    }
    if m.contains_key("factor") && m.contains_key("expr") && m.len() == 2 {
        let mut out = tx(format, &m["factor"], depth + 1)?;
        out.extend(tx(format, &m["expr"], depth + 1)?);
        return Ok(out);
    }
    if m.contains_key("operator") && m.contains_key("expr") && m.len() == 2 {
        let mut out = tx(format, &json!({"operator":m["operator"]}), depth + 1)?;
        out.extend(tx(format, &m["expr"], depth + 1)?);
        return Ok(out);
    }
    Err(incomplete(format!(
        "unported promised text capture keys {}",
        m.keys().cloned().collect::<Vec<_>>().join(",")
    )))
}
fn label(input: &str) -> Option<String> {
    // Source preprocess.rb masks '#' inside a greedy same-line colour span before split.
    let mut protected = vec![false; input.len()];
    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        if let (Some(a), Some(b)) = (line.find("✎("), line.rfind(')')) {
            if b > a && line[a..=b].contains('#') {
                for mark in protected.iter_mut().take(offset + b + 1).skip(offset + a) {
                    *mark = true
                }
            }
        }
        offset += line.len();
    }
    let mut pieces = Vec::new();
    let mut begin = 0;
    for (i, c) in input.char_indices() {
        if c == '#' && !protected[i] {
            pieces.push(&input[begin..i]);
            begin = i + 1
        }
    }
    pieces.push(&input[begin..]);
    while pieces.last() == Some(&"") {
        pieces.pop();
    }
    if pieces.len() > 1 {
        pieces.last().map(|s| s.to_string())
    } else {
        None
    }
}
pub fn parse(input: &str, format: &str, options: &Value) -> Result<Value, Value> {
    if !options.is_null() {
        let opts = options
            .as_object()
            .ok_or_else(|| error("INVALID_PARSE_OPTIONS", "parse options must be an object"))?;
        for (key, value) in opts {
            if key != "locale" {
                return Err(error(
                    "INCOMPLETE_PARSE_OPTIONS",
                    format!("unported promised parse option {key}"),
                ));
            }
            if !value.is_null() && value != "en" {
                return Err(error(
                    "INCOMPLETE_PARSE_OPTIONS",
                    "non-English locale conversion is not yet ported",
                ));
            }
        }
    }
    let raw=match format{
        "latex"=>LATEX.with(|cell|{let mut cell=cell.borrow_mut();if cell.is_none(){*cell=Some(latexmath::Parser::new().map_err(|e|error("FORMAT_BACKEND_INITIALIZATION",e))?)}let parsed=cell.as_ref().unwrap().parse_legacy(input).map_err(|e|json!({"code":"PARSE_ERROR","message":e.to_string(),"byte_offset":e.byte_offset(),"offset_policy":"caller_utf8_bytes_when_reported"}))?;untag(&parsed.canonical())}),
        "unicode"=>UNICODE.with(|cell|{let mut cell=cell.borrow_mut();if cell.is_none(){*cell=Some(umath::Parser::new().map_err(|e|error("FORMAT_BACKEND_INITIALIZATION",e))?)}let parsed=cell.as_ref().unwrap().parse_legacy(input).map_err(|e|json!({"code":"PARSE_ERROR","message":e.to_string(),"byte_offset":e.byte_offset(),"offset_policy":"preprocessed_utf8_bytes_when_reported"}))?;untag(&parsed.canonical())}),
        _=>Err(incomplete(format!("unported text input format {format}"))),
    }?;
    let mut values = tx(format, &raw, 0)?;
    if format == "unicode" {
        if let Some(id) = label(input) {
            let value = one(values);
            values = vec![node(
                "Math::Function::Mlabeledtr",
                json!({"parameter_one":value,"parameter_two":node("Math::Function::Text",json!({"lang":null,"parameter_one":id}))}),
            )]
        }
    }
    let mut result = formula(values);
    result["fields"]["input_string"] = json!(input);
    Ok(result)
}
