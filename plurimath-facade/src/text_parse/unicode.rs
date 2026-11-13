//! UnicodeMath capture rules ported from the source rule table
//! (plurimath 0a1df68 lib/plurimath/unicode_math/mapper/rules.rb).
//! Each rule matches an exact capture key set, as the source transform does;
//! any other shape falls through to the shared converter.
use super::{error, fenced, incomplete};
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};

type Rec<'a> = &'a dyn Fn(&Value) -> Result<Vec<Value>, Value>;

fn keys(m: &Map<String, Value>, want: &[&str]) -> bool {
    m.len() == want.len() && want.iter().all(|k| m.contains_key(*k))
}
fn symbol(text: &str) -> Value {
    crate::symbols::symbol_for(text, "unicode")
}
fn paren(name: &str) -> Value {
    node(
        &format!("Math::Symbols::Paren::{name}"),
        json!({"value":null}),
    )
}
/// Source Utility.filter_values on a captured list.
fn filter_values(mut values: Vec<Value>) -> Value {
    match values.len() {
        0 => Value::Null,
        1 => values.pop().unwrap(),
        _ => formula(values),
    }
}
/// Source Utility.unfenced_value(object, paren_specific: true) on one value.
fn unfenced(value: Value) -> Value {
    let f = &value["fields"];
    if value["class"] == "Math::Function::Fenced"
        && f["parameter_one"]["class"] == "Math::Symbols::Paren::Lround"
        && f["parameter_three"]["class"] == "Math::Symbols::Paren::Rround"
        && f["options"].as_object().is_some_and(Map::is_empty)
    {
        if let Some(body) = f["parameter_two"].as_array() {
            return filter_values(body.clone());
        }
    }
    value
}
/// unfenced_value on a captured value: a list is filtered, one value unfenced.
fn unf(mut values: Vec<Value>) -> Value {
    if values.len() == 1 {
        unfenced(values.pop().unwrap())
    } else {
        filter_values(values)
    }
}
/// A source `simple` binding: exactly one built value.
fn simple(mut values: Vec<Value>, what: &str) -> Result<Value, Value> {
    if values.len() == 1 {
        Ok(values.pop().unwrap())
    } else {
        Err(incomplete(format!(
            "unported UnicodeMath {what} capture sequence shape"
        )))
    }
}

const SUP_DIGITS: [&str; 10] = [
    "&#x2070;", "&#xb9;", "&#xb2;", "&#xb3;", "&#x2074;", "&#x2075;", "&#x2076;", "&#x2077;",
    "&#x2078;", "&#x2079;",
];
const SUB_DIGITS: [&str; 10] = [
    "&#x2080;", "&#x2081;", "&#x2082;", "&#x2083;", "&#x2084;", "&#x2085;", "&#x2086;", "&#x2087;",
    "&#x2088;", "&#x2089;",
];
/// rule(sup_digits:) / rule(sub_digits:): Constants::*_DIGITS.key(digits).to_s.
fn mini_digit(digits: &Value, sup: bool) -> Result<Value, Value> {
    let text = digits
        .as_str()
        .ok_or_else(|| incomplete("non-text mini digit capture"))?;
    let table = if sup { &SUP_DIGITS } else { &SUB_DIGITS };
    let digit = table
        .iter()
        .position(|d| *d == text)
        .map_or(String::new(), |i| i.to_string());
    let mut value = number(&digit);
    value["fields"][if sup {
        "mini_sup_sized"
    } else {
        "mini_sub_sized"
    }] = json!(true);
    Ok(value)
}

/// Source UnicodeMath::Utility.fractions.
fn fractions(
    numerator: Vec<Value>,
    denominator: Vec<Value>,
    options: Value,
) -> Result<Value, Value> {
    let numerator = unf(numerator);
    let denominator = if denominator.len() == 1 {
        denominator.into_iter().next().unwrap()
    } else {
        filter_values(denominator)
    };
    if denominator["class"] == "Math::Function::Frac" {
        if denominator["fields"]["parameter_one"]["class"] == "Math::Function::Frac" {
            return Err(incomplete("unported UnicodeMath recursive fraction shape"));
        }
        let mut outer = denominator;
        let inner = unfenced(outer["fields"]["parameter_one"].clone());
        outer["fields"]["parameter_one"] = node(
            "Math::Function::Frac",
            json!({"parameter_one":numerator,"parameter_two":inner,"options":options}),
        );
        return Ok(outer);
    }
    Ok(node(
        "Math::Function::Frac",
        json!({"parameter_one":numerator,"parameter_two":unfenced(denominator),"options":options}),
    ))
}

fn frac(m: &Map<String, Value>, rec: Rec) -> Result<Option<Value>, Value> {
    let operator = ["atop", "choose", "bevelled", "ldiv", "no_display_style"]
        .into_iter()
        .find(|k| keys(m, &["numerator", *k, "denominator"]));
    if let Some(kind) = operator {
        let n = rec(&m["numerator"])?;
        let d = rec(&m["denominator"])?;
        let (n_simple, d_simple) = (n.len() == 1, d.len() == 1);
        return Ok(Some(match kind {
            "atop" if d_simple => fractions(n, d, json!({"linethickness":"0"}))?,
            "choose" if n_simple && d_simple => fenced(
                paren("Lround"),
                vec![fractions(n, d, json!({"linethickness":"0","choose":true}))?],
                paren("Rround"),
            ),
            "bevelled" if n_simple => fractions(n, d, json!({"bevelled":true}))?,
            "ldiv" if n_simple => fractions(n, d, json!({"ldiv":true}))?,
            "no_display_style" if n_simple => {
                let options = if d_simple {
                    json!({"displaystyle":false})
                } else {
                    json!({"no_display_style":false})
                };
                fractions(n, d, options)?
            }
            _ => {
                return Err(incomplete(format!(
                    "unported UnicodeMath {kind} fraction capture shape"
                )))
            }
        }));
    }
    if keys(m, &["mini_numerator", "mini_denominator"]) {
        let n = rec(&m["mini_numerator"])?;
        let d = rec(&m["mini_denominator"])?;
        return Ok(Some(fractions(n, d, json!({"displaystyle":false}))?));
    }
    Ok(None)
}

const MATRIXS: [(&str, &str, &str); 8] = [
    ("pmatrix", "&#x24a8;", "⒨"),
    ("vmatrix", "&#x24b1;", "⒱"),
    ("Vmatrix", "&#x24a9;", "⒩"),
    ("bmatrix", "&#x24e2;", "ⓢ"),
    ("Bmatrix", "&#x24c8;", "Ⓢ"),
    ("eqarray", "&#x2588;", "█"),
    ("matrix", "&#x25a0;", "■"),
    ("cases", "&#x24b8;", "Ⓒ"),
];
/// rule(matrixs:, array:) and rule(matrixs:, identity_matrix_number:).
fn table(m: &Map<String, Value>, rows: Vec<Value>) -> Result<Value, Value> {
    let selector = m["matrixs"]
        .as_str()
        .ok_or_else(|| incomplete("non-text matrix selector"))?;
    let name = MATRIXS
        .iter()
        .find(|(_, entity, ch)| *entity == selector || *ch == selector)
        .map(|(name, _, _)| *name)
        .ok_or_else(|| incomplete(format!("unported UnicodeMath matrix {selector}")))?;
    // Class defaults from Math::Function::Table::* initializers, run through
    // Utility.symbols_class(lang: :unicodemath) by Table#initialize.
    let (class, open, close) = match name {
        "pmatrix" => ("Table::Pmatrix", symbol("("), symbol(")")),
        "vmatrix" => ("Table::Vmatrix", symbol("|"), symbol("|")),
        "Vmatrix" => ("Table::Vmatrix", paren("Norm"), symbol("|")),
        "bmatrix" => ("Table::Bmatrix", symbol("["), symbol("]")),
        "Bmatrix" => ("Table::Bmatrix", paren("Lcurly"), paren("Rcurly")),
        "eqarray" => ("Table::Eqarray", symbol(""), symbol("")),
        "cases" => ("Table::Cases", symbol("{"), symbol(":}")),
        _ => ("Table", Value::Null, Value::Null),
    };
    Ok(node(
        &format!("Math::Function::{class}"),
        json!({"value":rows,"open_paren":open,"close_paren":close,"options":{}}),
    ))
}
fn td(values: Vec<Value>) -> Value {
    node(
        "Math::Function::Td",
        json!({"parameter_one":values,"parameter_two":null}),
    )
}
fn tr(values: Vec<Value>) -> Value {
    node("Math::Function::Tr", json!({"parameter_one":values}))
}
/// Utility.identity_matrix.
fn identity(size: &Value) -> Result<Vec<Value>, Value> {
    let text = size
        .as_str()
        .ok_or_else(|| incomplete("non-text identity matrix size"))?;
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let n: usize = if digits.is_empty() {
        0
    } else {
        digits
            .parse()
            .map_err(|_| error("RESOURCE_LIMIT", "identity matrix size exceeds 256"))?
    };
    if n > 256 {
        return Err(error("RESOURCE_LIMIT", "identity matrix size exceeds 256"));
    }
    Ok((0..n)
        .map(|i| {
            tr((0..n)
                .map(|j| td(vec![number(if i == j { "1" } else { "0" })]))
                .collect())
        })
        .collect())
}

/// Multiscript pre-script rules (source rules over pre_subscript/pre_supscript).
fn pre_script(m: &Map<String, Value>, rec: Rec) -> Result<Option<Value>, Value> {
    if !m.contains_key("base")
        || !(m.contains_key("pre_subscript") || m.contains_key("pre_supscript"))
        || !m.keys().all(|k| {
            [
                "open_paren",
                "close_paren",
                "pre_subscript",
                "pre_supscript",
                "base",
                "sub",
                "sup",
                "sub_digits",
            ]
            .contains(&k.as_str())
        })
    {
        return Ok(None);
    }
    let parens = m.contains_key("open_paren") || m.contains_key("close_paren");
    if parens && !(m.contains_key("open_paren") && m.contains_key("close_paren")) {
        return Ok(None);
    }
    let has = |k: &str| m.contains_key(k);
    let one = |k: &str| -> Result<Value, Value> { simple(rec(&m[k])?, k) };
    let (pre_sub, pre_sup, sub, sup, digits) = (
        has("pre_subscript"),
        has("pre_supscript"),
        has("sub"),
        has("sup"),
        has("sub_digits"),
    );
    // (unfence pre scripts, unfence sub, unfence sup) per matching source rule.
    let shape = match (parens, pre_sub, pre_sup, sub, sup, digits) {
        (false, false, true, false, false, false) => (false, false, false),
        (false, true, false, false, false, false) => (false, false, false),
        (false, false, true, true, false, false) => (false, false, false),
        (false, true, true, false, false, false) => (false, false, false),
        (false, true, false, true, false, false) => (false, true, false),
        (false, true, false, false, false, true) => (false, false, false),
        (false, true, true, true, false, false) => (true, true, false),
        (false, true, true, true, true, false) => (true, true, true),
        (true, true, false, false, false, false) => (false, false, false),
        (true, true, false, true, false, false) => (false, false, false),
        (true, true, true, false, false, false) => (false, false, false),
        (true, true, true, true, true, false) => (false, true, true),
        _ => {
            return Err(incomplete(format!(
                "unported UnicodeMath pre-script capture keys {}",
                m.keys().cloned().collect::<Vec<_>>().join(",")
            )))
        }
    };
    let script = |k: &str, unfence: bool| -> Result<Vec<Value>, Value> {
        if !has(k) {
            return Ok(Vec::new());
        }
        let value = one(k)?;
        Ok(vec![if unfence { unfenced(value) } else { value }])
    };
    let base = one("base")?;
    let sub_value = if digits {
        mini_digit(&m["sub_digits"], false)?
    } else if sub {
        let value = one("sub")?;
        if shape.1 {
            unfenced(value)
        } else {
            value
        }
    } else {
        Value::Null
    };
    let sup_value = if sup {
        let value = one("sup")?;
        if shape.2 {
            unfenced(value)
        } else {
            value
        }
    } else {
        Value::Null
    };
    Ok(Some(node(
        "Math::Function::Multiscript",
        json!({
            "parameter_one": node("Math::Function::PowerBase", json!({"parameter_one":base,"parameter_two":sub_value,"parameter_three":sup_value})),
            "parameter_two": script("pre_subscript", shape.0)?,
            "parameter_three": script("pre_supscript", shape.0)?,
        }),
    )))
}

const NARY_CLASSES: [(&str, &str); 4] = [
    ("prod", "&#x220f;"),
    ("oint", "&#x222e;"),
    ("int", "&#x222b;"),
    ("sum", "&#x2211;"),
];
const NARY_SYMBOLS: [(&str, &str); 25] = [
    ("amalg", "&#x2210;"),
    ("aoint", "&#x2233;"),
    ("bigcap", "&#x22c2;"),
    ("bigcup", "&#x22c3;"),
    ("bigodot", "&#x2a00;"),
    ("bigoplus", "&#x2a01;"),
    ("bigotimes", "&#x2a02;"),
    ("bigsqcap", "&#x2a05;"),
    ("bigsqcup", "&#x2a06;"),
    ("bigudot", "&#x2a00;"),
    ("biguplus", "&#x2a04;"),
    ("bigvee", "&#x22c1;"),
    ("bigwedge", "&#x22c0;"),
    ("coint", "&#x2232;"),
    ("coprod", "&#x2210;"),
    ("cwint", "&#x2231;"),
    ("iiiint", "&#x2a0c;"),
    ("iiint", "&#x222d;"),
    ("iint", "&#x222c;"),
    ("int", "&#x222b;"),
    ("oiiint", "&#x2230;"),
    ("oiint", "&#x222f;"),
    ("oint", "&#x222e;"),
    ("prod", "&#x220f;"),
    ("sum", "&#x2211;"),
];
/// Masked n-ary rules (nary_class + mask + sub and/or sup).
fn masked_nary(m: &Map<String, Value>, rec: Rec) -> Result<Option<Value>, Value> {
    if !(m.contains_key("nary_class")
        && m.contains_key("mask")
        && m.keys()
            .all(|k| ["nary_class", "mask", "sub", "sup"].contains(&k.as_str()))
        && (m.contains_key("sub") || m.contains_key("sup")))
    {
        return Ok(None);
    }
    let class = m["nary_class"]
        .as_str()
        .ok_or_else(|| incomplete("non-text n-ary class"))?;
    let function = if NARY_CLASSES.iter().any(|(k, _)| *k == class) {
        class.to_string()
    } else if let Some((k, _)) = NARY_CLASSES.iter().find(|(_, v)| *v == class) {
        k.to_string()
    } else if let Some((_, v)) = NARY_SYMBOLS.iter().find(|(k, _)| *k == class) {
        v.to_string()
    } else {
        class.to_string()
    };
    let ternary = NARY_CLASSES.iter().any(|(k, _)| *k == function);
    let mask = simple(rec(&m["mask"])?, "mask")?;
    if mask["class"] != "Math::Number" {
        return Err(incomplete("unported UnicodeMath n-ary mask value"));
    }
    let mask = mask["fields"]["value"].clone();
    let sub = m.get("sub").map(rec).transpose()?;
    let sup = m.get("sup").map(rec).transpose()?;
    // Source has no sup-only sequence rule.
    if sub.is_none() && sup.as_ref().is_some_and(|v| v.len() != 1) {
        return Err(incomplete("unported UnicodeMath masked n-ary sup sequence"));
    }
    let both_simple = matches!((&sub, &sup), (Some(a), Some(b)) if a.len() == 1 && b.len() == 1);
    let side = |values: Option<Vec<Value>>, wrapper: &str, check: bool| -> Value {
        let Some(values) = values else {
            return Value::Null;
        };
        if values.len() != 1 {
            return filter_values(values);
        }
        let value = values.into_iter().next().unwrap();
        if check && value["class"] == wrapper {
            return value["fields"]["parameter_one"].clone();
        }
        unfenced(value)
    };
    let sub_check = sup.is_none() || both_simple;
    let mixed = sub.is_some() && sup.is_some() && !both_simple;
    let sub_value = side(sub, "Math::Function::Underset", sub_check);
    let sup_value = side(sup, "Math::Function::Overset", both_simple);
    if ternary {
        // Source passes options as the third positional argument when a side is
        // a sequence; that shape is not ported.
        if mixed {
            return Err(incomplete(
                "unported UnicodeMath masked n-ary sequence script shape",
            ));
        }
        let mut name = function.clone();
        name[..1].make_ascii_uppercase();
        return Ok(Some(node(
            &format!("Math::Function::{name}"),
            json!({"parameter_one":sub_value,"parameter_two":sup_value,"parameter_three":null,"options":{"mask":mask}}),
        )));
    }
    Ok(Some(node(
        "Math::Function::Nary",
        json!({"parameter_one":symbol(&function),"parameter_two":sub_value,"parameter_three":sup_value,"parameter_four":null,"options":{"mask":mask}}),
    )))
}

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if m.len() == 1 {
        let (key, value) = m.iter().next().unwrap();
        match key.as_str() {
            "table" | "pre_script" | "exp" | "nary" | "trs" => return Ok(Some(rec(value)?)),
            "td" => return Ok(Some(vec![td(rec(value)?)])),
            "tr" => return Ok(Some(vec![tr(rec(value)?)])),
            "sup_digits" => return Ok(Some(vec![mini_digit(value, true)?])),
            "sub_digits" => return Ok(Some(vec![mini_digit(value, false)?])),
            _ => {}
        }
    }
    if keys(m, &["td", "tds"]) {
        let mut out = vec![td(rec(&m["td"])?)];
        out.extend(rec(&m["tds"])?);
        return Ok(Some(out));
    }
    if keys(m, &["tr", "trs"]) {
        let mut out = vec![tr(rec(&m["tr"])?)];
        out.extend(rec(&m["trs"])?);
        return Ok(Some(out));
    }
    if keys(m, &["matrixs", "array"]) {
        return Ok(Some(vec![table(m, rec(&m["array"])?)?]));
    }
    if keys(m, &["matrixs", "identity_matrix_number"]) {
        return Ok(Some(vec![table(
            m,
            identity(&m["identity_matrix_number"])?,
        )?]));
    }
    if keys(m, &["factor", "exp"]) {
        let mut out = rec(&m["factor"])?;
        out.extend(rec(&m["exp"])?);
        return Ok(Some(out));
    }
    if keys(m, &["operator", "exp"]) {
        let operator = m["operator"]
            .as_str()
            .ok_or_else(|| incomplete("non-text operator capture"))?;
        let mut out = vec![symbol(operator)];
        out.extend(rec(&m["exp"])?);
        return Ok(Some(out));
    }
    if keys(m, &["nary_sub_sup", "naryand"]) {
        let mut nary = simple(rec(&m["nary_sub_sup"])?, "n-ary")?;
        let naryand = rec(&m["naryand"])?;
        let ternary = ["Sum", "Prod", "Int", "Oint"]
            .iter()
            .any(|k| nary["class"] == format!("Math::Function::{k}"));
        if ternary {
            nary["fields"]["parameter_three"] = if naryand.len() == 1 {
                naryand.into_iter().next().unwrap()
            } else {
                filter_values(naryand)
            };
            return Ok(Some(vec![nary]));
        }
        if nary["class"] != "Math::Function::Nary" {
            return Err(incomplete("unported UnicodeMath n-ary operand shape"));
        }
        if naryand.len() == 1 {
            nary["fields"]["parameter_four"] = naryand.into_iter().next().unwrap();
        } else if nary["fields"]["parameter_four"].is_null() {
            nary["fields"]["parameter_four"] = filter_values(naryand);
        }
        return Ok(Some(vec![nary]));
    }
    if let Some(value) = frac(m, rec)? {
        return Ok(Some(vec![value]));
    }
    if let Some(value) = pre_script(m, rec)? {
        return Ok(Some(vec![value]));
    }
    if let Some(value) = masked_nary(m, rec)? {
        return Ok(Some(vec![value]));
    }
    Ok(None)
}
