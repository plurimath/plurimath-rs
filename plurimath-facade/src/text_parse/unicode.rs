//! UnicodeMath capture rules ported from the source rule table
//! (plurimath 0a1df68 lib/plurimath/unicode_math/mapper/rules.rb).
//! Each rule matches an exact capture key set, as the source transform does;
//! any other shape falls through to the shared converter.
use super::{error, fenced, incomplete};
mod decorations;
mod heads;
mod intent;
mod structural_scripts;
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
pub(super) fn unfenced(value: Value) -> Value {
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
/// Source `simple` vs `sequence` binding of a built script: one value is bound
/// as is, several are a list.
fn bound(mut values: Vec<Value>) -> Result<Value, Vec<Value>> {
    if values.len() == 1 {
        Ok(values.pop().unwrap())
    } else {
        Err(values)
    }
}
/// Source Utility.base_is_prime?: the Power exponent is a prime symbol.
fn base_is_prime(power: &Value) -> bool {
    const PRIMES: [&str; 5] = ["&#x27;", "&#x2032;", "&#x2033;", "&#x2034;", "&#x2057;"];
    const CLASSES: [&str; 7] = [
        "Sprime", "Prime", "Dprime", "Pprime", "Ppprime", "Pppprime", "Qprime",
    ];
    let exponent = &power["fields"]["parameter_two"];
    let class = exponent["class"].as_str().unwrap_or("");
    CLASSES
        .iter()
        .any(|c| class == format!("Math::Symbols::{c}"))
        || exponent["fields"]["value"]
            .as_str()
            .is_some_and(|v| PRIMES.contains(&v))
}
/// Source Utility.base_is_sub_or_sup?.
fn base_is_sub_or_sup(value: &Value) -> bool {
    let f = &value["fields"];
    match value["class"].as_str().unwrap_or("") {
        "Math::Formula" => f["value"].get(0).is_some_and(base_is_sub_or_sup),
        "Math::Function::Fenced" => f["parameter_two"].get(0).is_some_and(base_is_sub_or_sup),
        "Math::Number" => f["mini_sub_sized"] == true || f["mini_sup_sized"] == true,
        c if c.starts_with("Math::Symbols::") => {
            f["mini_sub_sized"] == true || f["mini_sup_sized"] == true
        }
        _ => false,
    }
}
/// Source rules.rb 2125-2201: base with both scripts.
pub(super) fn sub_sup(base: Value, sub: Vec<Value>, sup: Vec<Value>) -> Value {
    let power_base = |base: Value, sub: Value, sup: Value| {
        node(
            "Math::Function::PowerBase",
            json!({"parameter_one":base,"parameter_two":sub,"parameter_three":sup}),
        )
    };
    match (bound(sub), bound(sup)) {
        (Ok(sub), Ok(sup)) => {
            let under_over = ["Math::Function::Underset", "Math::Function::Overset"];
            if under_over.iter().any(|c| sub["class"] == *c)
                && under_over.iter().any(|c| sup["class"] == *c)
            {
                return node(
                    "Math::Function::Underover",
                    json!({"parameter_one":base,"parameter_two":sub["fields"]["parameter_one"],"parameter_three":sup["fields"]["parameter_one"]}),
                );
            }
            power_base(base, unfenced(sub), unfenced(sup))
        }
        (Err(sub), Ok(sup)) => power_base(base, filter_values(sub), unfenced(sup)),
        // filter_values leaves a simple subscript as it is.
        (Ok(sub), Err(sup)) => power_base(base, sub, filter_values(sup)),
        (Err(sub), Err(sup)) => power_base(base, filter_values(sub), filter_values(sup)),
    }
}
/// Source rules.rb 1041-1074 and 1100-1114 after the binary-function and
/// bracket cases: an Underset takes the base, a primed Power becomes a PowerBase.
pub(super) fn sub_only(base: Value, sub: Vec<Value>) -> Value {
    let primed = base["class"] == "Math::Function::Power" && base_is_prime(&base);
    match bound(sub) {
        Ok(mut sub) if sub["class"] == "Math::Function::Underset" => {
            sub["fields"]["parameter_two"] = base;
            sub
        }
        Ok(sub) if primed => node(
            "Math::Function::PowerBase",
            json!({"parameter_one":base["fields"]["parameter_one"],"parameter_two":sub,"parameter_three":base["fields"]["parameter_two"]}),
        ),
        Err(sub) if primed => node(
            "Math::Function::PowerBase",
            json!({"parameter_one":base["fields"]["parameter_one"],"parameter_two":sub,"parameter_three":base["fields"]["parameter_two"]}),
        ),
        Ok(sub) => node(
            "Math::Function::Base",
            json!({"parameter_one":base,"parameter_two":unfenced(sub)}),
        ),
        Err(sub) => node(
            "Math::Function::Base",
            json!({"parameter_one":base,"parameter_two":filter_values(sub)}),
        ),
    }
}
/// Source rules.rb 1138-1159 and 1170-1184 after the overset and binary cases:
/// a sequence superscript over a mini-scripted Base completes a PowerBase.
pub(super) fn sup_only(base: Value, sup: Vec<Value>) -> Value {
    match bound(sup) {
        Ok(sup) => node(
            "Math::Function::Power",
            json!({"parameter_one":base,"parameter_two":unfenced(sup)}),
        ),
        Err(sup)
            if base["class"] == "Math::Function::Base"
                && base_is_sub_or_sup(&base["fields"]["parameter_two"]) =>
        {
            node(
                "Math::Function::PowerBase",
                json!({"parameter_one":base["fields"]["parameter_one"],"parameter_two":base["fields"]["parameter_two"],"parameter_three":filter_values(sup)}),
            )
        }
        Err(sup) => node(
            "Math::Function::Power",
            json!({"parameter_one":base,"parameter_two":filter_values(sup)}),
        ),
    }
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

/// HTMLEntities.decode for the entity forms the grammar captures.
fn decode_entities(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = tail.find(';').and_then(|e| {
            let body = &tail[1..e];
            let ch = if let Some(h) = body.strip_prefix("#x").or(body.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
            } else if let Some(d) = body.strip_prefix('#') {
                d.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match body {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ => None,
                }
            };
            ch.map(|c| (c, e + 1))
        });
        match decoded {
            Some((c, n)) => {
                out.push(c);
                rest = &tail[n..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
/// Source Utility.slashed_values on one captured value.
fn slashed_one(text: &str) -> Value {
    let decoded = decode_entities(text);
    let word = decoded
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    if word {
        text_node(&format!("\\{decoded}"))
    } else {
        node(
            "Math::Symbols::Symbol",
            json!({"slashed":true,"value":decoded}),
        )
    }
}
/// Source slashed_value rules, simple (one text) or sequence (symbol/number items).
fn slashed(value: &Value) -> Result<Vec<Value>, Value> {
    if let Some(t) = value.as_str() {
        return Ok(vec![slashed_one(t)]);
    }
    let items = value
        .as_array()
        .ok_or_else(|| incomplete("unported slashed_value capture shape"))?;
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let text = item
            .as_object()
            .and_then(|m| m.values().next())
            .and_then(Value::as_str)
            .ok_or_else(|| incomplete("unported slashed_value item shape"))?;
        let decoded = decode_entities(text);
        out.push(if i == 0 {
            slashed_one(text)
        } else if decoded.chars().any(|c| c.is_ascii_digit()) {
            number(&decoded)
        } else {
            symbol(&decoded)
        });
    }
    Ok(out)
}
fn text_node(text: &str) -> Value {
    node(
        "Math::Function::Text",
        json!({"lang":null,"parameter_one":text}),
    )
}
/// Source negated_operator / ordinary_negated_operator: the operator then a
/// combining long solidus overlay, in one Formula.
fn negated(operator: &Value) -> Result<Value, Value> {
    let operator = operator
        .as_str()
        .ok_or_else(|| incomplete("non-text negated operator capture"))?;
    Ok(formula(vec![
        symbol(operator),
        node("Math::Symbols::Symbol", json!({"value":"&#x338;"})),
    ]))
}
fn overset(over: Value, under: Value, accent: bool) -> Value {
    if accent {
        node(
            "Math::Function::Overset",
            json!({"options":{"accent":true},"parameter_one":over,"parameter_two":under}),
        )
    } else {
        node(
            "Math::Function::Overset",
            json!({"parameter_one":over,"parameter_two":under}),
        )
    }
}
/// Source rules.rb 351-359 build `accents` only when `expr` is a sequence. A
/// single-valued `expr` (a lone symbol, or a nested unbuilt capture) matches no
/// rule, so Parslet leaves the capture Hash with its children transformed.
/// Returns the ordered key/value pairs of that Hash, or `None` when the source
/// rule builds the list.
fn unbuilt_accents(
    m: &Map<String, Value>,
    tail: &str,
    rec: Rec,
) -> Result<Option<Vec<(String, Value)>>, Value> {
    let Some(items) = m["accents"].as_array() else {
        return Ok(None);
    };
    let expr = &m[tail];
    let expr_value = match expr.as_object() {
        Some(inner) if keys(inner, &["accents", tail]) => {
            match unbuilt_accents(inner, tail, rec)? {
                Some(pairs) => Value::Object(pairs.into_iter().collect()),
                None => return Ok(None),
            }
        }
        _ => {
            let mut values = rec(expr)?;
            if values.len() != 1 || expr.is_array() {
                return Ok(None);
            }
            values.pop().unwrap()
        }
    };
    let mut accents = Vec::new();
    for item in items {
        let item = item
            .as_object()
            .ok_or_else(|| incomplete("unported UnicodeMath accent capture shape"))?;
        if keys(item, &["first_value"]) {
            let mut values = rec(&item["first_value"])?;
            let value = if values.len() == 1 {
                values.pop().unwrap()
            } else {
                Value::Array(values)
            };
            accents.push(json!({"first_value": value}));
        } else if keys(item, &["accent_symbols"]) && item["accent_symbols"].is_string() {
            accents.push(Value::Object(item.clone()));
        } else {
            return Err(incomplete("unported UnicodeMath accent capture shape"));
        }
    }
    Ok(Some(vec![
        ("accents".to_string(), Value::Array(accents)),
        (tail.to_string(), expr_value),
    ]))
}
/// Source Utility.unicode_accents on a captured accent list: the first item
/// carries the base value, each later accent wraps what precedes it.
fn accents(value: &Value, rec: Rec) -> Result<Value, Value> {
    if value.is_object() {
        // A prime accent capture already builds its Power; the source keeps a
        // BinaryFunction unchanged.
        return simple(rec(value)?, "accents");
    }
    let items = value
        .as_array()
        .ok_or_else(|| incomplete("non-list accents capture"))?;
    let first = items
        .first()
        .and_then(Value::as_object)
        .filter(|m| keys(m, &["first_value"]))
        .ok_or_else(|| incomplete("unported UnicodeMath accents base shape"))?;
    if items.len() < 2 {
        return Err(incomplete("unported UnicodeMath single accent capture"));
    }
    let mut head = rec(&first["first_value"])?;
    let mut current = head
        .pop()
        .ok_or_else(|| incomplete("empty accents base capture"))?;
    for item in &items[1..] {
        let accent = item
            .as_object()
            .filter(|m| keys(m, &["accent_symbols"]))
            .and_then(|m| m["accent_symbols"].as_str())
            .ok_or_else(|| incomplete("unported UnicodeMath accent capture shape"))?;
        current = overset(symbol(accent), unfenced(current), true);
    }
    if head.is_empty() {
        Ok(current)
    } else {
        head.push(current);
        Ok(formula(head))
    }
}
const UNARY_FUNCTIONS: &[&str] = &[
    "Arctan",
    "Coth",
    "Bar",
    "Arcsin",
    "Dim",
    "Cosh",
    "Abs",
    "Cos",
    "Arccos",
    "Gcd",
    "Ceil",
    "Ddot",
    "Deg",
    "Det",
    "Ker",
    "Floor",
    "Csc",
    "Cancel",
    "Limsup",
    "Liminf",
    "Cot",
    "Csch",
    "Mbox",
    "Longdiv",
    "Dot",
    "Exp",
    "Mpadded",
    "Min",
    "Hom",
    "Lub",
    "Glb",
    "Max",
    "Hat",
    "Norm",
    "Lg",
    "Lcm",
    "Left",
    "Linebreak",
    "None",
    "Ln",
    "Scarry",
    "Sinh",
    "Mglyph",
    "Phantom",
    "Ms",
    "Msgroup",
    "Scarries",
    "Sin",
    "Tr",
    "Overleftrightarrow",
    "Obrace",
    "Sqrt",
    "Tanh",
    "Right",
    "Tan",
    "Ubrace",
    "Msline",
    "Ul",
    "Sech",
    "Vec",
    "Sup",
    "Substack",
    "Text",
    "Tilde",
    "Sec",
    "Merror",
];
fn is_unary(value: &Value) -> bool {
    value["class"]
        .as_str()
        .and_then(|c| c.strip_prefix("Math::Function::"))
        .is_some_and(|c| UNARY_FUNCTIONS.contains(&c))
}
/// A captured open/close paren carrying a size prefix (`├1(`): the symbol and
/// the em size the source derives as 1.25 ** mask. `None` size is a bare prefix.
pub(super) struct Masked {
    pub symbol: Value,
    pub size: Option<String>,
}
pub(super) fn masked_paren(v: &Value) -> Result<Option<Masked>, Value> {
    let Some(m) = v.as_object() else {
        return Ok(None);
    };
    if !(m.contains_key("paren_open_prefix") || m.contains_key("paren_close_prefix")) {
        return Ok(None);
    }
    let paren = ["open_paren", "close_paren"]
        .iter()
        .find_map(|k| m.get(*k).and_then(Value::as_str))
        .ok_or_else(|| incomplete("unported UnicodeMath prefixed paren capture"))?;
    let size = match m.len() {
        2 => None,
        3 => {
            let mask = ["open_paren_mask", "close_paren_mask"]
                .iter()
                .find_map(|k| m.get(*k))
                .and_then(Value::as_object)
                .filter(|mask| keys(mask, &["number"]))
                .and_then(|mask| mask["number"].as_str())
                .ok_or_else(|| incomplete("unported UnicodeMath paren mask capture"))?;
            if mask.is_empty() {
                None
            } else {
                let digits: String = mask.chars().take_while(char::is_ascii_digit).collect();
                let exponent = digits.parse::<i32>().unwrap_or(0);
                let size = format!("{}", 1.25f64.powf(f64::from(exponent)));
                let size = if size.contains('.') || size.contains('e') {
                    size
                } else {
                    format!("{size}.0")
                };
                Some(format!("{size}em"))
            }
        }
        _ => return Err(incomplete("unported UnicodeMath prefixed paren capture")),
    };
    Ok(Some(Masked {
        symbol: symbol(paren),
        size,
    }))
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
        // Source recursion_fraction: descend the numerator chain to its first non-fraction.
        let mut outer = denominator;
        let mut slot = &mut outer;
        while slot["fields"]["parameter_one"]["class"] == "Math::Function::Frac" {
            slot = &mut slot["fields"]["parameter_one"];
        }
        let inner = unfenced(slot["fields"]["parameter_one"].clone());
        slot["fields"]["parameter_one"] = new_frac(numerator, inner, options);
        return Ok(outer);
    }
    Ok(new_frac(numerator, unfenced(denominator), options))
}
fn new_frac(numerator: Value, denominator: Value, options: Value) -> Value {
    let mut fields = json!({"parameter_one":numerator,"parameter_two":denominator});
    if !options.is_null() {
        fields["options"] = options;
    }
    node("Math::Function::Frac", fields)
}

fn frac(m: &Map<String, Value>, rec: Rec) -> Result<Option<Value>, Value> {
    if keys(m, &["numerator", "denominator"]) {
        let n = rec(&m["numerator"])?;
        let d = rec(&m["denominator"])?;
        return Ok(Some(fractions(n, d, Value::Null)?));
    }
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

/// Unmasked n-ary rules: nary_class with sub and/or sup, or with a naryand.
/// A ternary class (sum, prod, int, oint) builds its function; any other class
/// builds a Nary over its symbol. Shapes whose source rule is missing, or whose
/// source constructor call fails, are refused.
fn unmasked_nary(m: &Map<String, Value>, rec: Rec) -> Result<Option<Value>, Value> {
    let scripts = keys(m, &["nary_class", "sub"])
        || keys(m, &["nary_class", "sup"])
        || keys(m, &["nary_class", "sub", "sup"]);
    if !scripts && !keys(m, &["nary_class", "naryand"]) {
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
    let script =
        |key: &str| -> Result<Option<Vec<Value>>, Value> { m.get(key).map(rec).transpose() };
    // Source `simple` bindings only; sequence operands are not ported here.
    let single = |values: Option<Vec<Value>>, what: &str| -> Result<Option<Value>, Value> {
        values.map(|values| simple(values, what)).transpose()
    };
    // Source `sub.class_name == "underset" ? sub.parameter_N : unfenced_value(sub)`.
    let unwrap = |value: Value, wrapper: &str, field: &str| -> Value {
        if value["class"] == wrapper {
            value["fields"][field].clone()
        } else {
            unfenced(value)
        }
    };
    let name = || {
        let mut name = function.clone();
        name[..1].make_ascii_uppercase();
        format!("Math::Function::{name}")
    };
    if let Some(naryand) = single(script("naryand")?, "n-ary naryand")? {
        return Ok(Some(if ternary {
            node(
                &name(),
                json!({"parameter_one":null,"parameter_two":null,"parameter_three":unwrap(naryand, "Math::Function::Underset", "parameter_two")}),
            )
        } else {
            node(
                "Math::Function::Nary",
                json!({"parameter_one":symbol(&function),"parameter_two":null,"parameter_three":null,"parameter_four":naryand,"options":{}}),
            )
        }));
    }
    let sub = script("sub")?;
    let sup = script("sup")?;
    let many = |values: &Option<Vec<Value>>| values.as_ref().is_some_and(|v| v.len() > 1);
    // Source `sequence` bindings (rules.rb 1941, 2205, 2867, 2896) for the
    // ternary classes; a superscript-only sequence has no source rule.
    let sub_many = many(&sub);
    let sequence = ternary && (sub_many || many(&sup));
    if let (true, Some(sub)) = (sequence, sub.clone()) {
        let sub = if sub_many {
            filter_values(sub)
        } else {
            unwrap(
                simple(sub, "n-ary sub")?,
                "Math::Function::Underset",
                "parameter_one",
            )
        };
        let sup = sup.map_or(Value::Null, filter_values);
        return Ok(Some(node(
            &name(),
            json!({"parameter_one":sub,"parameter_two":sup,"parameter_three":null}),
        )));
    }
    let sub = single(sub, "n-ary sub")?;
    let sup = single(sup, "n-ary sup")?;
    if ternary {
        let sub = sub.map_or(Value::Null, |v| {
            unwrap(v, "Math::Function::Underset", "parameter_one")
        });
        let sup = sup.map_or(Value::Null, |v| {
            unwrap(v, "Math::Function::Overset", "parameter_one")
        });
        return Ok(Some(node(
            &name(),
            json!({"parameter_one":sub,"parameter_two":sup,"parameter_three":null}),
        )));
    }
    if sub.is_none() {
        // Source calls the n-ary symbol constructor with (nil, sup), which raises;
        // Math.parse reports that as its ParseError.
        return Err(error(
            "PARSE_ERROR",
            "source n-ary symbol constructor rejects a superscript-only shape",
        ));
    }
    Ok(Some(node(
        "Math::Function::Nary",
        json!({"parameter_one":symbol(&function),"parameter_two":sub.map_or(Value::Null, unfenced),"parameter_three":sup.map_or(Value::Null, unfenced),"parameter_four":null,"options":{}}),
    )))
}

/// A captured cell: an empty cell captures its bare `&`, which the source Td
/// constructor deletes, leaving an empty cell.
fn cell(value: &Value, rec: Rec) -> Result<Vec<Value>, Value> {
    if value == "&" {
        return Ok(Vec::new());
    }
    rec(value)
}

pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if let Some(values) = intent::tx(m, rec)? {
        return Ok(Some(values));
    }
    if let Some(values) = decorations::tx(m, rec)? {
        return Ok(Some(values));
    }
    if let Some(values) = structural_scripts::tx(m, rec)? {
        return Ok(Some(values));
    }
    if let Some(values) = heads::tx(m, rec)? {
        return Ok(Some(values));
    }
    if m.len() == 1 {
        let (key, value) = m.iter().next().unwrap();
        match key.as_str() {
            "table" | "pre_script" | "exp" | "nary" | "trs" => return Ok(Some(rec(value)?)),
            // Source `nary_sub_sup: simple` returns the built n-ary unchanged.
            "nary_sub_sup" => return Ok(Some(vec![simple(rec(value)?, "n-ary")?])),
            // Source `unary_sub_sup: simple` also preserves the built function.
            "unary_sub_sup" => return Ok(Some(vec![simple(rec(value)?, "unary")?])),
            // Source unary_functions builds these binary classes with nil slots.
            "unary_functions" if matches!(value.as_str(), Some("lim" | "inf" | "log")) => {
                let class = match value.as_str().unwrap() {
                    "lim" => "Lim",
                    "inf" => "Inf",
                    _ => "Log",
                };
                return Ok(Some(vec![node(
                    &format!("Math::Function::{class}"),
                    json!({"parameter_one":null,"parameter_two":null}),
                )]));
            }
            "negated_operator" | "ordinary_negated_operator" => {
                return Ok(Some(vec![negated(value)?]))
            }
            // Source simple/sequence pass-through rules (rules.rb lines 22-98).
            "char" | "unary_function" | "rect" | "color" | "backcolor" | "mini_sup"
            | "mini_sub" | "mini_sub_sup" | "diacritics_accents" | "fonts" | "hbrack"
            | "diacritic_belows" | "diacritic_overlays" | "sup_recursion" => {
                return Ok(Some(rec(value)?))
            }
            "spaces" => {
                let name = value
                    .as_str()
                    .ok_or_else(|| incomplete("non-text spaces capture"))?;
                let space = match name {
                    "vthicksp" => "&#x2004;",
                    "thicksp" => "&#x2005;",
                    "thinsp" => "&#x2006;",
                    "hairsp" => "&#x200a;",
                    "numsp" => "&#x2007;",
                    "quad" | "emsp" => "&#x2003;",
                    "ensp" => "&#x2002;",
                    "nbsp" => "&#xa0;",
                    other => other,
                };
                return Ok(Some(vec![node(
                    "Math::Symbols::Symbol",
                    json!({"options":{"space":true},"value":space}),
                )]));
            }
            "accents" => return Ok(Some(vec![accents(value, rec)?])),
            "slashed_value" => return Ok(Some(slashed(value)?)),
            "td" => return Ok(Some(vec![td(cell(value, rec)?)])),
            "tr" => return Ok(Some(vec![tr(rec(value)?)])),
            "sup_digits" => return Ok(Some(vec![mini_digit(value, true)?])),
            "sub_digits" => return Ok(Some(vec![mini_digit(value, false)?])),
            _ => {}
        }
    }
    for (head, tail) in [("text", "expr"), ("text", "operand")] {
        if keys(m, &[head, tail]) {
            let text = m[head]
                .as_str()
                .ok_or_else(|| incomplete("non-text text capture"))?;
            let mut out = vec![text_node(text)];
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
    }
    // Source rules.rb 3604-3672: the text-led fence bodies of ⓘ("text"…) read an
    // operator tail as `exp`, optionally after one `operand`.
    for tail in [&["exp"][..], &["operand", "exp"][..]] {
        let mut want = vec!["text"];
        want.extend_from_slice(tail);
        if keys(m, &want) {
            let text = m["text"]
                .as_str()
                .ok_or_else(|| incomplete("non-text text capture"))?;
            let mut out = vec![text_node(text)];
            for key in tail {
                out.extend(rec(&m[*key])?);
            }
            return Ok(Some(out));
        }
    }
    // Source rules.rb 575-587: a subscript digit run is a mini-sized Number
    // followed by its recursion, whether that is one value or a sequence.
    if keys(m, &["sub_digits", "sub_recursion_expr"]) {
        let mut out = vec![mini_digit(&m["sub_digits"], false)?];
        out.extend(rec(&m["sub_recursion_expr"])?);
        return Ok(Some(out));
    }
    for head in ["negated_operator", "ordinary_negated_operator"] {
        if keys(m, &[head, "expr"]) {
            let mut out = vec![negated(&m[head])?];
            out.extend(rec(&m["expr"])?);
            return Ok(Some(out));
        }
    }
    for tail in ["expr", "naryand_recursion"] {
        if keys(m, &["nary", tail]) {
            let mut out = vec![simple(rec(&m["nary"])?, "n-ary")?];
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
    }
    // umath-rs parses `a^k b^(n-k)` as factor + pre-script where the source grammar
    // yields two sup_exp nodes; rebuild those so the source sup_exp rules apply.
    if keys(m, &["factor", "pre_script"]) {
        if let Some(p) = m["pre_script"].as_object() {
            if keys(p, &["pre_supscript", "base", "sup"]) {
                let first = json!({"base": m["factor"], "sup": {"sup_script": {"operand": p["pre_supscript"]}}});
                let second = json!({"base": p["base"], "sup": p["sup"]});
                let mut out = rec(&first)?;
                out.extend(rec(&second)?);
                return Ok(Some(out));
            }
        }
    }
    for tail in ["sup_exp", "sub_exp", "pre_script", "unary_subsup"] {
        if keys(m, &["factor", tail]) {
            let mut out = rec(&m["factor"])?;
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
    }
    // Source `[head] + tail` rules: the head capture is kept as is, then the tail list.
    const HEADS: [&str; 19] = [
        "diacritics_accents",
        "mini_sub_sup",
        "sub_exp",
        "frac",
        "subsup_exp",
        "override_subsup",
        "unary_subsup",
        "fonts",
        "phantom",
        "pre_script",
        "accents_subsup",
        "monospace",
        "mini_sub",
        "mini_sup",
        "expression",
        "unary_function",
        "rect",
        "table",
        "digit",
    ];
    if m.len() == 2 && m.contains_key("slashed_value") {
        for tail in ["expr", "exp"] {
            if m.contains_key(tail) {
                let mut out = slashed(&m["slashed_value"])?;
                out.extend(rec(&m[tail])?);
                return Ok(Some(out));
            }
        }
    }
    for head in HEADS {
        for tail in ["expr", "exp", "naryand_recursion"] {
            if keys(m, &[head, tail]) {
                let mut out = rec(&m[head])?;
                out.extend(rec(&m[tail])?);
                return Ok(Some(out));
            }
        }
    }
    // Source rules.rb 476-494: a unicode symbol heads its `exp`/`expr` tail.
    for tail in ["exp", "expr"] {
        if keys(m, &["unicode_symbols", tail]) {
            let mut out = vec![symbol(&capture_text(
                &m["unicode_symbols"],
                "unicode symbol",
            )?)];
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
    }
    // Source rules.rb 294-304: the binary symbol heads its recursive denominator.
    if keys(m, &["binary_symbols", "recursive_denominator"]) {
        let text = capture_text(&m["binary_symbols"], "binary symbol")?;
        let mut out = vec![symbol(&text)];
        out.extend(rec(&m["recursive_denominator"])?);
        return Ok(Some(out));
    }
    // Source rules.rb 2369-2373 and 2381-2385: three simple values in order.
    for head in ["factor", "intermediate_exp"] {
        if keys(m, &[head, "expr", "expression"]) {
            return Ok(Some(vec![
                simple(rec(&m[head])?, head)?,
                simple(rec(&m["expr"])?, "expr")?,
                simple(rec(&m["expression"])?, "expression")?,
            ]));
        }
    }
    // Source sequence rules over pass-through captures concatenate in grammar order.
    const PASS: [&str; 12] = [
        "atom",
        "atoms",
        "factor",
        "sub_exp",
        "sup_exp",
        "expr",
        "exp",
        "naryand_recursion",
        "operand",
        "recursive_numerator",
        "recursive_denominator",
        "digit",
    ];
    if m.len() >= 2 && m.keys().all(|k| PASS.contains(&k.as_str())) {
        // Source rules.rb 757-777: an operand follows its factor.
        let rank = |k: &str| match k {
            "atom" => 0,
            "factor" | "digit" => 1,
            "operand" => 2,
            "sub_exp" | "sup_exp" => 3,
            "exp" | "atoms" => 4,
            "recursive_numerator" => 5,
            "recursive_denominator" => 6,
            _ => 7,
        };
        let mut ordered: Vec<(&String, &Value)> = m.iter().collect();
        ordered.sort_by_key(|(k, _)| rank(k));
        let mut out = Vec::new();
        for (_, value) in ordered {
            out.extend(rec(value)?);
        }
        return Ok(Some(out));
    }
    if keys(m, &["first_value", "prime_accent_symbols"]) {
        let primes = m["prime_accent_symbols"]
            .as_str()
            .ok_or_else(|| incomplete("non-text prime accent capture"))?;
        let mut syms = Vec::new();
        let mut rest = primes;
        while let Some(start) = rest.find("&#x") {
            let tail = &rest[start..];
            let end = tail
                .find(';')
                .ok_or_else(|| incomplete("unterminated prime entity"))?;
            syms.push(symbol(&tail[..=end]));
            rest = &tail[end + 1..];
        }
        let first = unf(rec(&m["first_value"])?);
        return Ok(Some(vec![node(
            "Math::Function::Power",
            json!({"parameter_one":first,"parameter_two":filter_values(syms)}),
        )]));
    }
    if keys(m, &["unary_symbols", "first_value"]) {
        return unary_symbols(m, rec).map(Some);
    }
    if keys(m, &["rect_value", "first_value"]) {
        let mask = capture_text(&m["rect_value"], "rect mask")?;
        let attrs = enclosure_attrs(mask.trim().parse::<i64>().unwrap_or(0))?;
        return Ok(Some(vec![node(
            "Math::Function::Menclose",
            json!({"parameter_one":attrs,"parameter_two":unf(rec(&m["first_value"])?)}),
        )]));
    }
    for (key, backcolor) in [("color_value", false), ("backcolor_value", true)] {
        if keys(m, &[key, "first_value"]) {
            let color = capture_text(&m[key], "color")?;
            let body = rec(&m["first_value"])?;
            let body = if body.len() == 1 {
                body.into_iter().next().unwrap()
            } else {
                filter_values(body)
            };
            let color = node("Math::Symbols::Symbol", json!({"value":color}));
            let fields = if backcolor {
                json!({"options":{"backgroundcolor":true},"parameter_one":color,"parameter_two":body})
            } else {
                json!({"parameter_one":color,"parameter_two":body})
            };
            return Ok(Some(vec![node("Math::Function::Color", fields)]));
        }
    }
    for body_key in ["first_value", "scripted_first_value"] {
        if keys(m, &["hbracket_class", body_key]) {
            let hbrack = capture_text(&m["hbracket_class"], "bracket")?;
            let value = unf(rec(&m[body_key])?);
            return Ok(Some(vec![hbracket(
                &hbrack,
                value,
                body_key == "scripted_first_value",
            )]));
        }
    }
    if keys(m, &["factor", "operand", "expr"]) {
        let mut out = rec(&m["factor"])?;
        out.extend(rec(&m["operand"])?);
        out.extend(rec(&m["expr"])?);
        return Ok(Some(out));
    }
    for tail in ["expr", "exp"] {
        if keys(m, &["accents", tail]) {
            if let Some(pairs) = unbuilt_accents(m, tail, rec)? {
                // A Formula over a Hash holds its [key, value] pairs.
                return Ok(Some(
                    pairs.into_iter().map(|(k, v)| json!([k, v])).collect(),
                ));
            }
            let mut out = vec![accents(&m["accents"], rec)?];
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
        if keys(m, &["sup_exp", tail]) {
            let mut out = rec(&m["sup_exp"])?;
            out.extend(rec(&m[tail])?);
            return Ok(Some(out));
        }
    }
    if keys(m, &["over", "sup_script"]) {
        return Ok(Some(vec![overset(
            unf(rec(&m["sup_script"])?),
            Value::Null,
            false,
        )]));
    }
    if keys(m, &["unary_sub_sup", "first_value"]) {
        let mut sub_sup = simple(rec(&m["unary_sub_sup"])?, "unary")?;
        let first = rec(&m["first_value"])?;
        if is_unary(&sub_sup) {
            if !sub_sup["fields"]["parameter_one"].is_object() {
                return Err(incomplete("unported UnicodeMath unary operand shape"));
            }
            sub_sup["fields"]["parameter_one"]["fields"]["parameter_one"] = filter_values(first);
            return Ok(Some(vec![sub_sup]));
        }
        let mut out = vec![sub_sup];
        out.extend(first);
        return Ok(Some(vec![formula(out)]));
    }
    if keys(m, &["td", "tds"]) {
        let mut out = vec![td(cell(&m["td"], rec)?)];
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
    if let Some(value) = unmasked_nary(m, rec)? {
        return Ok(Some(vec![value]));
    }
    // Source rules.rb 99 and 215-222: a vulgar fraction character, then any tail.
    if let Some(text) = m.get("unicode_fractions") {
        let tail = ["expr", "exp"].into_iter().find(|k| m.contains_key(*k));
        if m.len() == 1 || (m.len() == 2 && tail.is_some()) {
            let text = text
                .as_str()
                .ok_or_else(|| incomplete("non-text unicode fraction capture"))?;
            let mut out = vec![unicode_fraction(text)?];
            if let Some(tail) = tail {
                out.extend(rec(&m[tail])?);
            }
            return Ok(Some(out));
        }
    }
    // Source rules.rb 3328-3337: a symbol heads the rest of a fence body.
    if keys(m, &["unicode_symbols", "exp"]) {
        let mut out = vec![leaf(m, "unicode_symbols")?];
        out.extend(rec(&m["exp"])?);
        return Ok(Some(out));
    }
    // Source rules.rb 3928-3939: monospace, arrow, its operand, then the rest.
    if keys(m, &["monospace", "relational_symbols", "expr", "exp"]) {
        let mut out = rec(&m["monospace"])?;
        out.push(leaf(m, "relational_symbols")?);
        out.extend(rec(&m["expr"])?);
        out.extend(rec(&m["exp"])?);
        return Ok(Some(out));
    }
    // Source rules.rb 3984-4036: an interval body; a list side is filtered.
    if keys(m, &["left_value", "comma", "right_value"]) {
        return Ok(Some(vec![
            filter_values(rec(&m["left_value"])?),
            leaf(m, "comma")?,
            filter_values(rec(&m["right_value"])?),
        ]));
    }
    // Source rules.rb 1583-1589: a phantom with a mask pads its value.
    if keys(m, &["phantom_value", "first_value"]) {
        let mask = m["phantom_value"]
            .as_str()
            .ok_or_else(|| incomplete("non-text phantom mask capture"))?;
        let value = simple(rec(&m["first_value"])?, "phantom")?;
        return Ok(Some(vec![node(
            "Math::Function::Mpadded",
            json!({"options":{"mask":mask},"parameter_one":value}),
        )]));
    }
    if keys(m, &["phantom"]) {
        return Ok(Some(rec(&m["phantom"])?));
    }
    Ok(None)
}
fn leaf(m: &Map<String, Value>, key: &str) -> Result<Value, Value> {
    m[key]
        .as_str()
        .map(symbol)
        .ok_or_else(|| incomplete(format!("non-text {key} capture")))
}
/// Source UnicodeMath::Utility.unicode_fractions over UNICODE_FRACTIONS.
fn unicode_fraction(text: &str) -> Result<Value, Value> {
    const FRACTIONS: [(&str, &str, &str); 18] = [
        ("&#x2153;", "1", "3"),
        ("&#x2154;", "2", "3"),
        ("&#x2155;", "1", "5"),
        ("&#x2156;", "2", "5"),
        ("&#x2157;", "3", "5"),
        ("&#x2158;", "4", "5"),
        ("&#x2159;", "1", "6"),
        ("&#x215a;", "5", "6"),
        ("&#x2150;", "1", "7"),
        ("&#x215b;", "1", "8"),
        ("&#x215c;", "3", "8"),
        ("&#x215d;", "5", "8"),
        ("&#x215e;", "7", "8"),
        ("&#x2151;", "1", "9"),
        ("&#x2189;", "0", "3"),
        ("&#xbd;", "1", "2"),
        ("&#xbc;", "1", "4"),
        ("&#xbe;", "3", "4"),
    ];
    let (_, n, d) = FRACTIONS
        .iter()
        .find(|(k, _, _)| *k == text)
        .ok_or_else(|| incomplete("unported unicode fraction character"))?;
    Ok(new_frac(
        number(n),
        number(d),
        json!({"displaystyle":false,"unicodemath_fraction":true}),
    ))
}

fn capture_text(value: &Value, what: &str) -> Result<String, Value> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| incomplete(format!("non-text {what} capture")))
}
/// Source Utility.enclosure_attrs.
fn enclosure_attrs(mask: i64) -> Result<String, Value> {
    if !(0..=255).contains(&mask) {
        return Err(incomplete("enclosure mask is not between 0 and 255"));
    }
    let mask = mask ^ 15;
    let names = [
        "top",
        "bottom",
        "left",
        "right",
        "horizontalstrike",
        "verticalstrike",
        "downdiagonalstrike",
        "updiagonalstrike",
    ];
    Ok((0..8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| names[i])
        .collect::<Vec<_>>()
        .join(" "))
}
/// Source `unary_symbols, first_value` rule: phantom/smash or menclose.
fn unary_symbols(m: &Map<String, Value>, rec: Rec) -> Result<Vec<Value>, Value> {
    const UNARY: [(&str, &str); 13] = [
        ("underline", "&#x2581;"),
        ("hphantom", "&#x2b04;"),
        ("vphantom", "&#x21f3;"),
        ("underbar", "&#x2581;"),
        ("overline", "&#xaf;"),
        ("phantom", "&#x27e1;"),
        ("longdiv", "&#x27cc;"),
        ("circle", "&#x25cb;"),
        ("asmash", "&#x2b06;"),
        ("dsmash", "&#x2b07;"),
        ("hsmash", "&#x2b0c;"),
        ("smash", "&#x2b0d;"),
        ("overbar", "&#xaf;"),
    ];
    const MENCLOSE: [(&str, &str); 10] = [
        ("underline", "bottom"),
        ("underbar", "bottom"),
        ("longdiv", "longdiv"),
        ("xcancel", "updiagonalstrike downdiagonalstrike"),
        ("bcancel", "updiagonalstrike"),
        ("ellipse", "circle"),
        ("circle", "circle"),
        ("cancel", "downdiagonalstrike"),
        ("rrect", "roundedbox"),
        ("rect", "box"),
    ];
    let unary = capture_text(&m["unary_symbols"], "unary symbol")?;
    let name = UNARY
        .iter()
        .find(|(_, v)| *v == unary)
        .map_or(unary.as_str(), |(k, _)| *k);
    let first = unf(rec(&m["first_value"])?);
    // Source PHANTOM_SYMBOLS entry: (mpadded options, phantom flag).
    let phantom: Option<(Option<Value>, bool)> = match name {
        "hphantom" => Some((Some(json!({"depth":"0","height":"0"})), true)),
        "vphantom" => Some((Some(json!({"width":"0"})), true)),
        "phantom" => Some((None, true)),
        "hsmash" => Some((Some(json!({"width":"0"})), false)),
        "asmash" => Some((Some(json!({"height":"0"})), false)),
        "dsmash" => Some((Some(json!({"depth":"0"})), false)),
        "smash" => Some((Some(json!({"height":"0","depth":"0"})), false)),
        _ => None,
    };
    if let Some((mpadded, wrap)) = phantom {
        let mut value = first;
        if let Some(options) = mpadded {
            // The source stores the whole PHANTOM_SYMBOLS entry as the options.
            value = node(
                "Math::Function::Mpadded",
                json!({"options":{"mpadded":options,"phantom":wrap},"parameter_one":value}),
            );
        }
        if wrap {
            value = node("Math::Function::Phantom", json!({"parameter_one":value}));
        }
        return Ok(vec![value]);
    }
    let notation = MENCLOSE
        .iter()
        .find(|(k, _)| *k == unary)
        .or_else(|| MENCLOSE.iter().find(|(k, _)| *k == name))
        .map(|(_, v)| *v);
    Ok(vec![node(
        "Math::Function::Menclose",
        json!({"parameter_one":notation,"parameter_two":first}),
    )])
}
/// Source `hbracket_class` rules (first_value and scripted_first_value).
fn hbracket(hbrack: &str, value: Value, scripted: bool) -> Value {
    let bracket = |name: &str, v: Value| {
        node(
            &format!("Math::Function::{name}"),
            json!({"attributes":{},"parameter_one":v}),
        )
    };
    let symbol = node("Math::Symbols::Symbol", json!({"value":hbrack}));
    match hbrack {
        "&#x23de;" => bracket("Obrace", value),
        "&#x23df;" => {
            let f = &value["fields"];
            if value["class"] == "Math::Function::Base"
                && f["parameter_one"]["class"] != "Math::Formula"
            {
                let inner = if scripted {
                    unfenced(f["parameter_one"].clone())
                } else {
                    f["parameter_one"].clone()
                };
                node(
                    "Math::Function::Underset",
                    json!({"options":{},"parameter_one":f["parameter_two"],"parameter_two":bracket("Ubrace", inner)}),
                )
            } else {
                bracket("Ubrace", value)
            }
        }
        "&#x23b5;" | "&#x23e1;" | "&#x23dd;" => node(
            "Math::Function::Underset",
            json!({"options":{},"parameter_one":symbol,"parameter_two":value}),
        ),
        _ => node(
            "Math::Function::Overset",
            json!({"parameter_one":symbol,"parameter_two":value}),
        ),
    }
}
