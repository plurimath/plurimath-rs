//! UnicodeMath list and leaf rules whose head is a captured symbol or a
//! built value followed by a tail, pinned to Ruby 0a1df68
//! (lib/plurimath/unicode_math/mapper/rules.rb, line numbers cited per rule).
use super::{capture_text, filter_values, keys, overset, simple, symbol, unf, Rec};
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};

use super::super::incomplete;

/// Constants::SUP_ALPHABETS.key(alpha).to_s
fn sup_alpha(text: &str) -> &'static str {
    const SUP_ALPHABETS: &[(&str, &str)] = &[
        ("a", "&#x1d43;"),
        ("b", "&#x1d47;"),
        ("c", "&#x1d9c;"),
        ("d", "&#x1d48;"),
        ("e", "&#x1d49;"),
        ("f", "&#x1da0;"),
        ("g", "&#x1d4d;"),
        ("h", "&#x2b0;"),
        ("i", "&#x2071;"),
        ("j", "&#x2b2;"),
        ("k", "&#x1d4f;"),
        ("l", "&#x2e1;"),
        ("m", "&#x1d50;"),
        ("n", "&#x207f;"),
        ("o", "&#x1d52;"),
        ("p", "&#x1d56;"),
        ("r", "&#x2b3;"),
        ("s", "&#x2e2;"),
        ("t", "&#x1d57;"),
        ("u", "&#x1d58;"),
        ("v", "&#x1d5b;"),
        ("w", "&#x2b7;"),
        ("x", "&#x2e3;"),
        ("y", "&#x2b8;"),
        ("z", "&#x1dbb;"),
    ];
    SUP_ALPHABETS
        .iter()
        .find(|(_, v)| *v == text)
        .map_or("", |(k, _)| *k)
}
/// Constants::SUP_OPERATORS / SUB_OPERATORS .key(operator).to_s
fn mini_operator(text: &str, sup: bool) -> &'static str {
    const SUP: &[(&str, &str)] = &[("+", "&#x207a;"), ("-", "&#x207b;"), ("=", "&#x207c;")];
    const SUB: &[(&str, &str)] = &[
        ("+", "&#x208a;"),
        ("-", "&#x208b;"),
        ("=", "&#x208c;"),
        ("\u{2094}", "&#x2094;"),
    ];
    let table: &[(&str, &str)] = if sup { SUP } else { SUB };
    table
        .iter()
        .find(|(_, v)| *v == text)
        .map_or("", |(k, _)| *k)
}
fn mini_symbol(value: &str, sup: bool) -> Value {
    let size = if sup {
        "mini_sup_sized"
    } else {
        "mini_sub_sized"
    };
    let mut fields = json!({ "value": value });
    fields[size] = json!(true);
    node("Math::Symbols::Symbol", fields)
}
/// Constants::SUP_DIGITS.key(digits).to_s as a mini-sup Number.
fn sup_digit(text: &str) -> Value {
    const SUP_DIGITS: &[&str] = &[
        "&#x2070;", "&#xb9;", "&#xb2;", "&#xb3;", "&#x2074;", "&#x2075;", "&#x2076;", "&#x2077;",
        "&#x2078;", "&#x2079;",
    ];
    let digit = SUP_DIGITS
        .iter()
        .position(|d| *d == text)
        .map_or(String::new(), |i| i.to_string());
    let mut value = number(&digit);
    value["fields"]["mini_sup_sized"] = json!(true);
    value
}
/// Utility.unicode_fractions.
fn unicode_fraction(text: &str) -> Result<Value, Value> {
    const FRACTIONS: &[(&str, u8, u8)] = &[
        ("&#x2153;", 1, 3),
        ("&#x2154;", 2, 3),
        ("&#x2155;", 1, 5),
        ("&#x2156;", 2, 5),
        ("&#x2157;", 3, 5),
        ("&#x2158;", 4, 5),
        ("&#x2159;", 1, 6),
        ("&#x215a;", 5, 6),
        ("&#x2150;", 1, 7),
        ("&#x215b;", 1, 8),
        ("&#x215c;", 3, 8),
        ("&#x215d;", 5, 8),
        ("&#x215e;", 7, 8),
        ("&#x2151;", 1, 9),
        ("&#x2189;", 0, 3),
        ("&#xbd;", 1, 2),
        ("&#xbc;", 1, 4),
        ("&#xbe;", 3, 4),
    ];
    let (_, n, d) = FRACTIONS
        .iter()
        .find(|(k, _, _)| *k == text)
        .ok_or_else(|| incomplete("unknown UnicodeMath vulgar fraction"))?;
    Ok(node(
        "Math::Function::Frac",
        json!({
            "options":{"displaystyle":false,"unicodemath_fraction":true},
            "parameter_one":number(&n.to_string()),
            "parameter_two":number(&d.to_string())
        }),
    ))
}
/// Constants::COMBINING_SYMBOLS[combined.to_sym]
fn combining(text: &str) -> Result<&'static str, Value> {
    match text {
        "!!" => Ok("&#x203c;"),
        "-+" => Ok("&#x2213;"),
        "+-" => Ok("&#xb1;"),
        _ => Err(incomplete("unported UnicodeMath combined symbol")),
    }
}
/// Constants::OVERLAYS_NOTATIONS
fn overlay_notation(text: &str) -> Option<&'static str> {
    const MOVER: &[&str] = &[
        "&#x20eb;", "&#x20ea;", "&#x20e6;", "&#x20e5;", "&#x20e4;", "&#x20e3;", "&#x20e2;",
        "&#x20df;", "&#x20da;", "&#x20d9;", "&#x20d4;", "&#x20d6;", "&#x20d5;", "&#x20d8;",
        "&#x20d3;", "&#x20d2;", "&#x338;", "&#x337;", "&#x336;", "&#x335;", "&#x334;", "&#x309;",
    ];
    match text {
        "&#x20e0;" => Some("circle downdiagonalstrike"),
        "&#x20de;" => Some("box"),
        "&#x20dd;" => Some("circle"),
        "&#x304;" => Some("top"),
        t if MOVER.contains(&t) => Some("mover"),
        _ => None,
    }
}
/// Constants::BELOWS_NOTATIONS ("bottom" for U+0332, else "munder" when listed).
fn below_notation(text: &str) -> Option<&'static str> {
    const MUNDER: &[&str] = &[
        "&#x316;", "&#x317;", "&#x318;", "&#x319;", "&#x31c;", "&#x31d;", "&#x31e;", "&#x31f;",
        "&#x320;", "&#x321;", "&#x322;", "&#x323;", "&#x324;", "&#x325;", "&#x326;", "&#x327;",
        "&#x328;", "&#x329;", "&#x32a;", "&#x32b;", "&#x32c;", "&#x32d;", "&#x32e;", "&#x32f;",
        "&#x330;", "&#x331;", "&#x333;", "&#x339;", "&#x33a;", "&#x33b;", "&#x33c;", "&#x345;",
        "&#x347;", "&#x348;", "&#x349;", "&#x34d;", "&#x34e;", "&#x353;", "&#x354;", "&#x355;",
        "&#x356;", "&#x359;", "&#x35a;", "&#x35c;", "&#x35f;", "&#x362;", "&#x20e8;", "&#x20ec;",
        "&#x20ed;", "&#x20ee;", "&#x20ef;",
    ];
    match text {
        "&#x332;" => Some("bottom"),
        t if MUNDER.contains(&t) => Some("munder"),
        _ => None,
    }
}
fn menclose(notation: Option<&str>, value: Value) -> Value {
    node(
        "Math::Function::Menclose",
        json!({"parameter_one":notation,"parameter_two":value}),
    )
}
/// Source rules.rb 1442-1490: an overlay over the last first_value item.
fn overlay(text: &str, value: Value) -> Value {
    let notation = overlay_notation(text);
    if notation == Some("mover") || text == "&#x304;" {
        overset(symbol(text), value, true)
    } else {
        menclose(notation, value)
    }
}
/// Source rules.rb 1513-1527.
fn below(text: &str, value: Value) -> Value {
    let notation = below_notation(text);
    if notation == Some("munder") {
        node(
            "Math::Function::Underset",
            json!({"options":{"accent":true},"parameter_one":symbol(text),"parameter_two":value}),
        )
    } else {
        menclose(notation, value)
    }
}
/// Source unary_arg_functions rule (rules.rb 1231-1245).
fn unary_arg(function: &str, value: Value) -> Value {
    if function == "abs" || function == "&#x249c;" {
        return node("Math::Function::Abs", json!({ "parameter_one": value }));
    }
    const NAMES: &[(&str, &str)] = &[
        ("bcancel", "&#x2572;"),
        ("xcancel", "&#x2573;"),
        ("ellipse", "&#x2b2d;"),
        ("cancel", "&#x2571;"),
        ("rrect", "&#x25a2;"),
        ("rect", "&#x25ad;"),
    ];
    let name = NAMES
        .iter()
        .find(|(_, v)| *v == function)
        .map_or(function, |(k, _)| *k);
    let notation = match name {
        "underline" | "underbar" => Some("bottom"),
        "longdiv" => Some("longdiv"),
        "xcancel" => Some("updiagonalstrike downdiagonalstrike"),
        "bcancel" => Some("updiagonalstrike"),
        "ellipse" | "circle" => Some("circle"),
        "cancel" => Some("downdiagonalstrike"),
        "rrect" => Some("roundedbox"),
        "rect" => Some("box"),
        _ => None,
    };
    menclose(notation, value)
}

/// Each `[head] + tail` rule whose head is one captured symbol: the head
/// capture key, the tail keys it pairs with, and the symbol text mapping.
const SYMBOL_HEADS: &[(&str, &[&str])] = &[
    // rules.rb 253-270, 312-317
    ("symbol", &["expr", "naryand_recursion"]),
    // rules.rb 521-530, 555-560
    ("operator", &["sub_script", "sup_script"]),
    // rules.rb 461-470
    ("mid_symbol", &["expr"]),
    // rules.rb 306-311
    // rules.rb 276-280, 288-292
    (
        "binary_symbols",
        &["recursive_numerator", "exp", "naryand_recursion"],
    ),
    // rules.rb 688-695
    ("relational_symbols", &["recursive_denominator"]),
    // rules.rb 531-535
    ("combined_symbols", &["sup_script"]),
    ("unicode_fractions", &["expr"]),
];

fn head_symbol(key: &str, text: &str) -> Result<Value, Value> {
    match key {
        "combined_symbols" => Ok(symbol(combining(text)?)),
        "unicode_fractions" => unicode_fraction(text),
        _ => Ok(symbol(text)),
    }
}

/// A source `rule(key: simple(:x)) { x }` applied to a one-key capture.
fn reduced<'a>(value: &'a Value, key: &str) -> &'a Value {
    match value.as_object() {
        Some(m) if m.len() == 1 && m.get(key).is_some_and(Value::is_string) => &m[key],
        _ => value,
    }
}

/// rules.rb 178-185 class resolution, limited to the ternary NARY_CLASSES.
fn bare_nary(class: &str) -> Option<String> {
    use super::{NARY_CLASSES, NARY_SYMBOLS};
    let function = if NARY_CLASSES.iter().any(|(k, _)| *k == class) {
        class
    } else if let Some((k, _)) = NARY_CLASSES.iter().find(|(_, v)| *v == class) {
        *k
    } else if let Some((_, v)) = NARY_SYMBOLS.iter().find(|(k, _)| *k == class) {
        *v
    } else {
        class
    };
    NARY_CLASSES
        .iter()
        .find(|(k, _)| *k == function)
        .map(|(k, _)| {
            let mut name = k.to_string();
            name[..1].make_ascii_uppercase();
            format!("Math::Function::{name}")
        })
}

/// Constants::UNDEF_UNARY_FUNCTIONS: names kept as plain symbols.
const UNDEF_UNARY: &[&str] = &["arg", "def", "erf", "Im", "Pr", "Re", "tg"];
/// Utility.get_class(name) for the one-slot UnaryFunction classes.
fn plain_unary(name: &str) -> Option<String> {
    const PLAIN: &[&str] = &[
        "Arccos", "Arcsin", "Arctan", "Cos", "Cosh", "Cot", "Coth", "Csc", "Csch", "Deg", "Det",
        "Dim", "Exp", "Gcd", "Glb", "Hom", "Ker", "Lcm", "Lg", "Liminf", "Limsup", "Ln", "Lub",
        "Max", "Min", "Sec", "Sech", "Sin", "Sinh", "Sup", "Tan", "Tanh", "Tr",
    ];
    let class: String = name
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| {
                    f.to_uppercase()
                        .chain(c.flat_map(char::to_lowercase))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join("");
    PLAIN
        .contains(&class.as_str())
        .then(|| format!("Math::Function::{class}"))
}

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if m.len() == 1 {
        let (key, value) = m.iter().next().unwrap();
        match key.as_str() {
            // rules.rb 129
            "unary_functions" => {
                if let Some(name) = value.as_str() {
                    if UNDEF_UNARY.contains(&name) {
                        return Ok(Some(vec![symbol(name)]));
                    }
                    if let Some(class) = plain_unary(name) {
                        return Ok(Some(vec![node(&class, json!({"parameter_one":null}))]));
                    }
                }
            }
            // rules.rb 102-105
            "combined_symbols" => {
                if let Some(text) = value.as_str() {
                    return Ok(Some(vec![symbol(combining(text).unwrap_or(text))]));
                }
            }
            // rules.rb 100
            "mini_intermediate_exp" => {
                return Ok(Some(vec![simple(rec(value)?, "mini intermediate")?]))
            }
            // rules.rb 178-185: a bare n-ary class builds with nil slots.
            "nary_class" => {
                if let Some(name) = value.as_str().and_then(bare_nary) {
                    return Ok(Some(vec![node(
                        &name,
                        json!({"parameter_one":null,"parameter_two":null,"parameter_three":null}),
                    )]));
                }
            }
            // rules.rb 57
            "sup_alpha" => {
                let text = capture_text(value, "superscript letter")?;
                return Ok(Some(vec![mini_symbol(sup_alpha(&text), true)]));
            }
            // rules.rb 116
            "sup_operators" => {
                let text = capture_text(value, "superscript operator")?;
                return Ok(Some(vec![mini_symbol(mini_operator(&text, true), true)]));
            }
            // rules.rb 95
            "exclamation_symbol" => {
                return Ok(Some(vec![symbol(&capture_text(value, "exclamation")?)]))
            }
            // rules.rb 100
            "unicode_fractions" => {
                let text = capture_text(value, "vulgar fraction")?;
                return Ok(Some(vec![unicode_fraction(&text)?]));
            }
            // rules.rb 72, 76, 81: pass-through captures
            "accents_subsup" | "override_subsup" | "monospace" => {
                return Ok(Some(vec![simple(rec(value)?, key)?]))
            }
            // rules.rb 156-166
            "monospace_value" => {
                let values = rec(value)?;
                let body = if values.len() == 1 && !value.is_array() {
                    values.into_iter().next().unwrap()
                } else {
                    filter_values(values)
                };
                return Ok(Some(vec![node(
                    "Math::Function::FontStyle::Monospace",
                    json!({"parameter_one":body,"parameter_two":"monospace"}),
                )]));
            }
            _ => {}
        }
    }
    // rules.rb 1569
    if keys(m, &["unary_functions", "first_value"]) {
        if let Some(name) = m["unary_functions"].as_str() {
            let first = rec(&m["first_value"])?;
            if first.len() == 1 && !first[0].is_array() {
                let first = first.into_iter().next().unwrap();
                if UNDEF_UNARY.contains(&name) {
                    return Ok(Some(vec![formula(vec![symbol(name), first])]));
                }
                if name == "mod" {
                    return Ok(Some(vec![node(
                        "Math::Function::Mod",
                        json!({"parameter_one":null,"parameter_two":first}),
                    )]));
                }
                if let Some(class) = plain_unary(name) {
                    return Ok(Some(vec![node(&class, json!({"parameter_one":first}))]));
                }
            }
        }
    }
    for &(head, tails) in SYMBOL_HEADS {
        for &tail in tails {
            if keys(m, &[head, tail]) {
                let text = capture_text(&m[head], head)?;
                let mut out = vec![head_symbol(head, &text)?];
                out.extend(rec(&m[tail])?);
                return Ok(Some(out));
            }
        }
    }
    // rules.rb 1015-1023, 1753-1756, 1828-1836: built values in rule order;
    // `true` marks a source `simple` binding.
    const CONCAT: &[&[(&str, bool)]] = &[
        &[("sup_script", true), ("mini_sup", true)],
        &[("sup_script", true), ("mini_sub", true)],
        &[("factor", true), ("mini_sup", true)],
        &[("expr", true), ("func_expr", false)],
    ];
    for &parts in CONCAT {
        let names: Vec<&str> = parts.iter().map(|(k, _)| *k).collect();
        if keys(m, &names) {
            let mut out = Vec::new();
            for &(key, single) in parts {
                let values = rec(&m[key])?;
                if single {
                    out.push(simple(values, key)?);
                } else {
                    out.extend(values);
                }
            }
            return Ok(Some(out));
        }
    }
    // rules.rb 1868-1871
    if keys(m, &["intermediate_exp", "exclamation_symbol"]) {
        let mut out = vec![simple(rec(&m["intermediate_exp"])?, "intermediate_exp")?];
        out.push(symbol(&capture_text(
            &m["exclamation_symbol"],
            "exclamation",
        )?));
        return Ok(Some(out));
    }
    // rules.rb 2423-2431
    if keys(m, &["fonts", "relational_symbols", "expr"]) {
        let mut out = vec![simple(rec(&m["fonts"])?, "fonts")?];
        out.push(symbol(&capture_text(
            &m["relational_symbols"],
            "relational symbol",
        )?));
        out.push(simple(rec(&m["expr"])?, "expr")?);
        return Ok(Some(out));
    }
    // rules.rb 2837-2844
    if keys(m, &["operator", "frac", "expr"]) {
        let mut out = vec![symbol(&capture_text(&m["operator"], "operator")?)];
        out.push(simple(rec(&m["frac"])?, "frac")?);
        out.extend(rec(&m["expr"])?);
        return Ok(Some(out));
    }
    // rules.rb 674-684: a mini-sup operator heads its recursions.
    if keys(m, &["sup_operators", "sup_recursions"]) {
        let text = capture_text(&m["sup_operators"], "superscript operator")?;
        let mut out = vec![mini_symbol(mini_operator(&text, true), true)];
        out.extend(rec(&m["sup_recursions"])?);
        return Ok(Some(out));
    }
    // rules.rb 2466-2474: the operator is a mini-sub Number, not a Symbol.
    if keys(
        m,
        &["mini_intermediate_exp", "sub_operators", "sub_recursions"],
    ) {
        let mut out = vec![simple(
            rec(&m["mini_intermediate_exp"])?,
            "mini intermediate",
        )?];
        let text = capture_text(&m["sub_operators"], "subscript operator")?;
        let mut op = number(mini_operator(&text, false));
        op["fields"]["mini_sub_sized"] = json!(true);
        out.push(op);
        out.extend(rec(&m["sub_recursions"])?);
        return Ok(Some(out));
    }
    // rules.rb 2144-2161: a size-overridden subscript base.
    if keys(m, &["base", "size_overrides", "sub_script"]) {
        let size = match capture_text(&m["size_overrides"], "size override")?.as_str() {
            "A" => "1.25em",
            "B" => "1.5625em",
            "C" => "0.8em",
            "D" => "0.64em",
            _ => return Err(incomplete("unknown UnicodeMath size override")),
        };
        let base = simple(rec(&m["base"])?, "base")?;
        let sub = rec(&m["sub_script"])?;
        let sub = if m["sub_script"].is_array() || sub.len() != 1 {
            filter_values(sub)
        } else {
            unf(sub)
        };
        return Ok(Some(vec![node(
            "Math::Function::Base",
            json!({"options":{"size":size},"parameter_one":base,"parameter_two":sub}),
        )]));
    }
    // rules.rb 2565-2585: a one-sided prefixed paren around a factor.
    for (open, close, prefixed) in [
        ("open_paren", "paren_close_prefix", "close_prefixed"),
        ("paren_open_prefix", "close_paren", "open_prefixed"),
    ] {
        if keys(m, &[open, "factor", close]) {
            if let (Some(o), Some(c)) = (m[open].as_str(), m[close].as_str()) {
                let factor = simple(rec(&m["factor"])?, "factor")?;
                let mut options = Map::new();
                options.insert(prefixed.to_string(), json!(true));
                return Ok(Some(vec![node(
                    "Math::Function::Fenced",
                    json!({"options":options,"parameter_one":symbol(o),"parameter_two":[factor],"parameter_three":symbol(c)}),
                )]));
            }
        }
    }
    // rules.rb 2637-2647: Constants::SUB_PARENTHESIS mini-sub parens.
    if keys(m, &["sub_open_paren", "mini_expr", "sub_close_paren"]) {
        let open = if m["sub_open_paren"].as_str() == Some("&#x208d;") {
            "("
        } else {
            ""
        };
        let close = if m["sub_close_paren"].as_str() == Some("&#x208e;") {
            ")"
        } else {
            ""
        };
        return Ok(Some(vec![node(
            "Math::Function::Fenced",
            json!({"options":{},"parameter_one":mini_symbol(open, false),"parameter_two":rec(&m["mini_expr"])?,"parameter_three":mini_symbol(close, false)}),
        )]));
    }
    // rules.rb 2801-2806: the table takes the paren and the close prefix.
    if keys(m, &["open_paren", "table", "paren_close_prefix"]) {
        if let (Some(o), Some(c)) = (m["open_paren"].as_str(), m["paren_close_prefix"].as_str()) {
            let mut table = simple(rec(&m["table"])?, "table")?;
            if table["class"]
                .as_str()
                .is_some_and(|class| class.starts_with("Math::Function::Table"))
            {
                table["fields"]["open_paren"] = symbol(o);
                table["fields"]["close_paren"] = symbol(c);
                return Ok(Some(vec![table]));
            }
        }
    }
    // rules.rb 1583-1589
    if keys(m, &["phantom_value", "first_value"]) {
        let mask = capture_text(&m["phantom_value"], "phantom mask")?;
        let first = simple(rec(&m["first_value"])?, "phantom operand")?;
        return Ok(Some(vec![node(
            "Math::Function::Mpadded",
            json!({"options":{"mask":mask},"parameter_one":first}),
        )]));
    }
    // rules.rb 662-673: a mini-sub operator heads its recursions.
    if keys(m, &["sub_operators", "sub_recursions"]) {
        let text = capture_text(&m["sub_operators"], "subscript operator")?;
        let mut out = vec![mini_symbol(mini_operator(&text, false), false)];
        out.extend(rec(&m["sub_recursions"])?);
        return Ok(Some(out));
    }
    // rules.rb 589-619: a mini-sup letter or digit heads its recursion.
    if keys(m, &["sup_alpha", "sup_recursion_expr"]) {
        let text = capture_text(&m["sup_alpha"], "superscript letter")?;
        let mut out = vec![mini_symbol(sup_alpha(&text), true)];
        out.extend(rec(&m["sup_recursion_expr"])?);
        return Ok(Some(out));
    }
    if keys(m, &["sup_digits", "sup_recursion_expr"]) {
        let text = capture_text(&m["sup_digits"], "superscript digits")?;
        let mut out = vec![sup_digit(&text)];
        out.extend(rec(&m["sup_recursion_expr"])?);
        return Ok(Some(out));
    }
    // rules.rb 451, 1858-1882: a built head then its expr tail.
    for head in ["intermediate_exp", "int_exp"] {
        if keys(m, &[head, "expr"]) {
            let mut out = vec![simple(rec(&m[head])?, head)?];
            out.extend(rec(&m["expr"])?);
            return Ok(Some(out));
        }
    }
    // rules.rb 1873-1876
    if keys(m, &["atom", "exclamation_symbol"]) {
        let mut out = vec![simple(rec(&m["atom"])?, "atom")?];
        out.push(symbol(&capture_text(
            &m["exclamation_symbol"],
            "exclamation",
        )?));
        return Ok(Some(out));
    }
    // rules.rb 2070-2075
    if keys(m, &["atom", "binary_symbols", "recursive_numerator"]) {
        let mut out = vec![simple(rec(&m["atom"])?, "atom")?];
        out.push(symbol(&capture_text(
            &m["binary_symbols"],
            "binary symbol",
        )?));
        out.extend(rec(&m["recursive_numerator"])?);
        return Ok(Some(out));
    }
    // rules.rb 1231-1245
    if keys(m, &["unary_arg_functions", "first_value"]) {
        let function = capture_text(&m["unary_arg_functions"], "unary function")?;
        let value = unf(rec(&m["first_value"])?);
        return Ok(Some(vec![unary_arg(&function, value)]));
    }
    // rules.rb 1442-1490
    if keys(m, &["first_value", "overlay_after"]) {
        // rules.rb 97 reduces a nested diacritic_overlays capture to its text first.
        let text = capture_text(
            reduced(&m["overlay_after"], "diacritic_overlays"),
            "overlay",
        )?;
        let mut values = rec(&m["first_value"])?;
        if values.len() == 1 && !m["first_value"].is_array() {
            let value = unf(values);
            return Ok(Some(vec![overlay(&text, value)]));
        }
        let last = values
            .pop()
            .ok_or_else(|| incomplete("empty overlay base capture"))?;
        values.push(overlay(&text, unf(vec![last])));
        return Ok(Some(vec![crate::model::formula(values)]));
    }
    // rules.rb 1495-1511: a leading overlay (simple bindings only).
    if keys(m, &["overlay_before", "first_value"]) {
        let text = capture_text(
            reduced(&m["overlay_before"], "diacritic_overlays"),
            "overlay",
        )?;
        let value = unf(vec![simple(rec(&m["first_value"])?, "overlay base")?]);
        // Unlike the trailing rule, U+0304 keeps its "top" notation here.
        let notation = overlay_notation(&text);
        return Ok(Some(vec![if notation == Some("mover") {
            overset(symbol(&text), value, true)
        } else {
            menclose(notation, value)
        }]));
    }
    // rules.rb 1513-1527
    if keys(m, &["first_value", "below_after"]) {
        // rules.rb 84 reduces a nested diacritic_belows capture to its text first.
        let text = capture_text(
            reduced(&m["below_after"], "diacritic_belows"),
            "below accent",
        )?;
        let value = unf(vec![simple(rec(&m["first_value"])?, "below base")?]);
        return Ok(Some(vec![below(&text, value)]));
    }
    // rules.rb 1536-1566: an indexed radical.
    for (first_key, second_key) in [
        ("first_value", "second_value"),
        ("root_first_value", "root_second_value"),
    ] {
        if keys(m, &[first_key, second_key]) {
            let first = filter_values(rec(&m[first_key])?);
            let second = unf(rec(&m[second_key])?);
            return Ok(Some(vec![node(
                "Math::Function::Root",
                json!({"parameter_one":first,"parameter_two":second}),
            )]));
        }
    }
    Ok(None)
}
