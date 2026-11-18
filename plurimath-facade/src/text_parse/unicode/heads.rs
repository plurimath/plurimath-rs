//! UnicodeMath list and leaf rules whose head is a captured symbol or a
//! built value followed by a tail, pinned to Ruby 0a1df68
//! (lib/plurimath/unicode_math/mapper/rules.rb, line numbers cited per rule).
use super::{capture_text, filter_values, keys, overset, simple, symbol, unf, Rec};
use crate::model::{node, number};
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
    ("binary_symbols", &["recursive_numerator"]),
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

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if m.len() == 1 {
        let (key, value) = m.iter().next().unwrap();
        match key.as_str() {
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
        let text = capture_text(&m["overlay_after"], "overlay")?;
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
    // rules.rb 1513-1527
    if keys(m, &["first_value", "below_after"]) {
        let text = capture_text(&m["below_after"], "below accent")?;
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
