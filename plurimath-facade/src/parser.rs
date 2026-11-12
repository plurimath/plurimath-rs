//! Source capture transformation over the accepted stage parser; no text grammar here.
use crate::model::{formula, node, number};
use serde_json::{json, Map, Value};
// Parsed::tree already applies stage flatten_shape, unwrap and Renester.
// Resolve only its source spans, directly on a heap work list; canonical tagged
// JSON is a diagnostic representation and need not be recursively materialized.
fn source_capture_tree(root: &Value, source: &str) -> Result<Value, String> {
    enum Work<'a> {
        Visit(&'a Value),
        Array(usize),
        Object(Vec<String>),
    }
    let mut work = vec![Work::Visit(root)];
    let mut output = Vec::<Value>::new();
    while let Some(task) = work.pop() {
        match task {
            Work::Visit(Value::Array(values)) => {
                work.push(Work::Array(values.len()));
                for value in values.iter().rev() {
                    work.push(Work::Visit(value));
                }
            }
            Work::Visit(Value::Object(fields)) => {
                let input_ref = fields.get("offset").is_some_and(Value::is_u64)
                    && fields.get("length").is_some_and(Value::is_u64)
                    && fields.keys().all(|k| {
                        ["offset", "length", "value", "line", "column"].contains(&k.as_str())
                    });
                if input_ref {
                    let text = if let Some(value) = fields.get("value").and_then(Value::as_str) {
                        value.to_owned()
                    } else {
                        let offset = fields["offset"].as_u64().unwrap() as usize;
                        let count = fields["length"].as_u64().unwrap() as usize;
                        source
                            .get(offset..)
                            .ok_or("stage source span is not a UTF8 byte boundary")?
                            .chars()
                            .take(count)
                            .collect()
                    };
                    output.push(Value::String(text));
                } else {
                    work.push(Work::Object(fields.keys().cloned().collect()));
                    for value in fields.values().rev() {
                        work.push(Work::Visit(value));
                    }
                }
            }
            Work::Visit(value) => output.push(value.clone()),
            Work::Array(count) => {
                let values = output.split_off(output.len() - count);
                output.push(Value::Array(values));
            }
            Work::Object(keys) => {
                let values = output.split_off(output.len() - keys.len());
                output.push(Value::Object(keys.into_iter().zip(values).collect()));
            }
        }
    }
    output
        .pop()
        .ok_or("stage capture lowering produced no value".into())
}
// Source AsciiMath's symbol registry treats the legacy delimiter sentinels
// as the corresponding delimiter tokens, not generic mathematical letters.
fn source_symbol(text: &str) -> Value {
    // Source Utility checks exact spellings first: "\\ " is Space, while
    // the stripped fallback "\\" is Backslash.
    if crate::symbols::tables()["input"][text].is_string() {
        return crate::symbols::symbol(text);
    }
    crate::symbols::symbol_for(
        match text {
            "ℒ" => "{:",
            "ℛ" => ":}",
            "ᑕ" => "(:",
            "ᑐ" => ":)",
            other => other,
        },
        "asciimath",
    )
}
fn single(mut a: Vec<Value>) -> Value {
    if a.is_empty() {
        Value::Null
    } else if a.len() == 1 {
        a.pop().unwrap()
    } else {
        formula(a)
    }
}
fn unfence(v: Value) -> Value {
    if v["class"] == "Math::Function::Fenced" {
        if v["fields"]["parameter_two"].is_null() {
            return Value::Null;
        }
        if let Some(a) = v["fields"]["parameter_two"].as_array() {
            return single(a.clone());
        }
    }
    v
}
#[derive(Debug)]
enum ConversionError {
    Unported(String),
    Resource { depth: usize, max_depth: usize },
}
impl From<String> for ConversionError {
    fn from(value: String) -> Self {
        Self::Unported(value)
    }
}
impl From<&str> for ConversionError {
    fn from(value: &str) -> Self {
        Self::Unported(value.into())
    }
}
fn tx_rule_0(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("table_left") && m.contains_key("table_right") && m.contains_key("table_row")
    {
        let left = m["table_left"]
            .as_str()
            .or_else(|| m["table_left"].get("table_left").and_then(Value::as_str))
            .ok_or("unported source table-left shape")?;
        let right = m["table_right"]
            .as_str()
            .or_else(|| m["table_right"].get("table_right").and_then(Value::as_str))
            .ok_or("unported source table-right shape")?;
        let mut rows = tx(&m["table_row"], depth + 1)?;
        if let Some(rest) = m.get("expr") {
            rows.extend(tx(rest, depth + 1)?);
        }
        if rows.iter().any(|v| v["class"] != "Math::Function::Tr") {
            return Err("unported non-row source table content".into());
        }
        return Ok(Some(vec![node(
            "Math::Function::Table",
            json!({"options":{},"open_paren":source_symbol(left),"close_paren":source_symbol(right),"value":rows}),
        )]));
    }
    Ok(None)
}
fn tx_rule_1(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("open_tr") && m.contains_key("tds_list") {
        if m.len() != 2 {
            return Err("unported source table-row fields".into());
        }
        return Ok(Some(vec![node(
            "Math::Function::Tr",
            json!({"parameter_one":tx(&m["tds_list"],depth+1)?}),
        )]));
    }
    Ok(None)
}
fn tx_rule_2(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(value) = m.get("td") {
        if m.len() != 1 {
            return Err("unported combined source Td fields".into());
        }
        let contents = tx(value, depth + 1)?;
        let mut cells = vec![];
        let mut cell = vec![];
        for value in contents {
            if value["class"] == "Math::Symbols::Comma" {
                cells.push(node(
                    "Math::Function::Td",
                    json!({"parameter_one":cell,"parameter_two":null}),
                ));
                cell = vec![];
            } else {
                cell.push(value);
            }
        }
        cells.push(node(
            "Math::Function::Td",
            json!({"parameter_one":cell,"parameter_two":null}),
        ));
        return Ok(Some(cells));
    }
    Ok(None)
}
fn tx_rule_3(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(row) = m.get("table_row") {
        let mut rows = tx(row, depth + 1)?;
        if let Some(rest) = m.get("expr") {
            rows.extend(tx(rest, depth + 1)?);
        }
        if m.keys()
            .any(|k| !["table_row", "expr"].contains(&k.as_str()))
        {
            return Err("unported source table rows".into());
        }
        return Ok(Some(rows));
    }
    Ok(None)
}
fn tx_rule_4(m: &Map<String, Value>, _depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.len() == 2 && m.contains_key("d") && m.contains_key("x") {
        let d = m["d"].as_str().ok_or("non-text differential d")?;
        let x = m["x"].as_str().ok_or("non-text differential x")?;
        return Ok(Some(vec![formula(vec![
            source_symbol(d),
            source_symbol(x),
        ])]));
    }
    Ok(None)
}
fn tx_rule_5(m: &Map<String, Value>, _depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(text) = m.get("unitsml").and_then(Value::as_str) {
        if m.len() != 1 {
            return Err("unported combined UnitsML capture".into());
        }
        return Ok(Some(vec![node(
            "Math::Function::Unitsml",
            json!({"text":text}),
        )]));
    }
    Ok(None)
}
fn tx_rule_6(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(close) = m.get("rparen").and_then(Value::as_str) {
        if m.contains_key("lparen") { /* handled by the fenced rule below */
        } else {
            let mut out = vec![source_symbol(close)];
            if let Some(rest) = m.get("expr") {
                out.extend(tx(rest, depth + 1)?);
            }
            if m.keys().any(|k| !["rparen", "expr"].contains(&k.as_str())) {
                return Err("unported closing-paren script capture".into());
            }
            return Ok(Some(out));
        }
    }
    Ok(None)
}
fn tx_rule_7(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(name) = m.get("ternary_class").and_then(Value::as_str) {
        let class = crate::symbols::tables()["class_map"][name]
            .as_str()
            .ok_or("unknown source ternary class")?;
        let a = m
            .get("base_value")
            .or_else(|| m.get("base"))
            .map(|v| tx(v, depth + 1).map(single).map(unfence))
            .transpose()?
            .unwrap_or(Value::Null);
        let b = m
            .get("power_value")
            .or_else(|| m.get("power"))
            .map(|v| tx(v, depth + 1).map(single).map(unfence))
            .transpose()?
            .unwrap_or(Value::Null);
        let c = m
            .get("third_value")
            .filter(|v| !v.is_string())
            .map(|v| tx(v, depth + 1).map(single))
            .transpose()?
            .unwrap_or(Value::Null);
        if m.keys().any(|k| {
            ![
                "ternary_class",
                "base_value",
                "base",
                "power_value",
                "power",
                "third_value",
                "expr",
            ]
            .contains(&k.as_str())
        }) {
            return Err("unported source ternary fields".into());
        }
        let mut out = vec![node(
            class,
            json!({"parameter_one":a,"parameter_two":b,"parameter_three":c}),
        )];
        if let Some(rest) = m.get("expr") {
            out.extend(tx(rest, depth + 1)?);
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_8(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("dividend")
        && m.contains_key("divisor")
        && m.get("mod") == Some(&json!("mod"))
    {
        return Ok(Some(vec![node(
            "Math::Function::Mod",
            json!({"parameter_one":single(tx(&m["dividend"],depth+1)?),"parameter_two":single(tx(&m["divisor"],depth+1)?)}),
        )]));
    }
    Ok(None)
}
fn tx_rule_9(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(left) = m.get("left").and_then(Value::as_str) {
        let right = m
            .get("right")
            .and_then(Value::as_str)
            .ok_or("source right delimiter must be text")?;
        if m.len() != 3 || !m.contains_key("left_right_value") {
            return Err("unported left/right fields".into());
        }
        let inner = single(tx(&m["left_right_value"], depth + 1)?);
        let mut value = formula(vec![
            node("Math::Function::Left", json!({"parameter_one":left})),
            inner,
            node("Math::Function::Right", json!({"parameter_one":right})),
        ]);
        value["fields"]["left_right_wrapper"] = json!(false);
        return Ok(Some(vec![value]));
    }
    Ok(None)
}
fn tx_rule_10(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(font) = m.get("fonts_class").and_then(Value::as_str) {
        if !m.contains_key("fonts_value") {
            return Err("unported source font constructor without operand".into());
        }
        let class = crate::symbols::tables()["font_map"][font]
            .as_str()
            .ok_or_else(|| format!("unported source font {font}"))?;
        let arg = unfence(single(tx(&m["fonts_value"], depth + 1)?));
        let mut out = vec![node(
            class,
            json!({"parameter_one":arg,"parameter_two":font}),
        )];
        if let Some(rest) = m.get("expr") {
            out.extend(tx(rest, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_11(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("color") && m.contains_key("color_value") {
        let a = unfence(single(tx(&m["color"], depth + 1)?));
        let b = unfence(single(tx(&m["color_value"], depth + 1)?));
        let mut out = vec![node(
            "Math::Function::Color",
            json!({"parameter_one":a,"parameter_two":b}),
        )];
        if let Some(rest) = m.get("expr") {
            out.extend(tx(rest, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_12(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(t) = m.get("text") {
        let value = if let Some(s) = t.as_str() {
            s.to_owned()
        } else if let Some(a) = t.as_array() {
            let mut text = String::new();
            for s in a {
                text.push_str(s.as_str().ok_or("unported non-text literal capture")?)
            }
            text
        } else {
            return Err("unported source text capture".into());
        };
        let mut out = vec![node(
            "Math::Function::Text",
            json!({"parameter_one":value,"lang":null}),
        )];
        if let Some(rest) = m.get("expr") {
            out.extend(tx(rest, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_13(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(c) = m.get("comma") {
        let mut out = vec![];
        if let Some(value) = m.get("number") {
            if m.len() != 2 {
                return Err("unported source number/comma fields".into());
            }
            out.push(number(
                value.as_str().ok_or("number capture is not source text")?,
            ));
        } else if m.len() > 2
            || m.keys()
                .any(|key| !["comma", "expr", "left_right"].contains(&key.as_str()))
        {
            return Err("unported combined source comma fields".into());
        }
        out.push(source_symbol(c.as_str().ok_or("unported comma capture")?));
        if let Some(rest) = m.get("expr").or_else(|| m.get("left_right")) {
            out.extend(tx(rest, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_14(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("power_base") && m.contains_key("base") {
        if m.keys()
            .any(|k| !["power_base", "base", "expr"].contains(&k.as_str()))
        {
            return Err("unported combined source Base captures".into());
        }
        let a = single(tx(&m["power_base"], depth + 1)?);
        let b = unfence(single(tx(&m["base"], depth + 1)?));
        let mut out = vec![node(
            "Math::Function::Base",
            json!({"parameter_one":a,"parameter_two":b}),
        )];
        if let Some(rest) = m.get("expr") {
            out.extend(tx(rest, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
// Ruby 0a1df68 AsciiMath mapper rules 59, 676-705 and 724-730 call
// the declared binary constructor; Underset alone declares an empty options map.
fn source_binary_node(name: &str, first: Value, second: Value) -> Result<Value, ConversionError> {
    let class = crate::symbols::tables()["class_map"][name]
        .as_str()
        .ok_or("source binary class unavailable")?;
    let fields = if class == "Math::Function::Underset" {
        json!({"options":{},"parameter_one":first,"parameter_two":second})
    } else {
        json!({"parameter_one":first,"parameter_two":second})
    };
    Ok(node(class, fields))
}

fn tx_rule_15(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(name) = m.get("binary_class").and_then(Value::as_str) {
        // Source rule 854 emits two bare constructors, not a nested unary call.
        if m.len() == 2 && m.contains_key("unary_class") {
            let mut unary = Map::new();
            unary.insert("unary_class".into(), m["unary_class"].clone());
            let mut values =
                tx_rule_18(&unary, depth)?.ok_or("source unary constructor unavailable")?;
            values.push(source_binary_node(name, Value::Null, Value::Null)?);
            return Ok(Some(values));
        }
        if ["root", "log", "lim", "overset", "underset", "stackrel"].contains(&name) {
            if m.keys().any(|key| {
                ![
                    "binary_class",
                    "base_value",
                    "base",
                    "power_value",
                    "power",
                    "expr",
                ]
                .contains(&key.as_str())
            }) {
                return Err("unported combined source binary captures".into());
            }
            let a = if let Some(v) = m.get("base_value").or_else(|| m.get("base")) {
                unfence(single(tx(v, depth + 1)?))
            } else {
                Value::Null
            };
            let b = if let Some(v) = m.get("power_value").or_else(|| m.get("power")) {
                unfence(single(tx(v, depth + 1)?))
            } else {
                Value::Null
            };
            let mut out = vec![source_binary_node(name, a, b)?];
            if let Some(rest) = m.get("expr") {
                out.extend(tx(rest, depth + 1)?)
            }
            return Ok(Some(out));
        }
    }
    Ok(None)
}
fn tx_rule_16(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("power_base") && m.contains_key("base_value") && m.contains_key("power_value")
    {
        if m.len() != 3 {
            return Err("unported combined source PowerBase captures".into());
        }
        return Ok(Some(vec![node(
            "Math::Function::PowerBase",
            json!({"parameter_one":single(tx(&m["power_base"],depth+1)?),"parameter_two":unfence(single(tx(&m["base_value"],depth+1)?)),"parameter_three":unfence(single(tx(&m["power_value"],depth+1)?))}),
        )]));
    }
    Ok(None)
}
fn tx_rule_17(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("power_base") && m.contains_key("power") {
        if m.keys()
            .any(|k| !["power_base", "power", "expr"].contains(&k.as_str()))
        {
            return Err("unported combined source power captures".into());
        }
        let base = single(tx(&m["power_base"], depth + 1)?);
        let script = tx(&m["power"], depth + 1)?;
        if script.len() != 1 {
            return Err("unported source sequence-power constructor profile".into());
        }
        let exponent = unfence(single(script));
        let mut out = vec![node(
            "Math::Function::Power",
            json!({"parameter_one":base,"parameter_two":exponent}),
        )];
        if let Some(v) = m.get("expr") {
            out.extend(tx(v, depth + 1)?)
        }
        return Ok(Some(out));
    }
    Ok(None)
}
fn tx_rule_18(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(name) = m.get("unary_class").and_then(Value::as_str) {
        let class = crate::symbols::tables()["class_map"][name]
            .as_str()
            .ok_or_else(|| format!("unported source function name {name}"))?;
        let mut rest = m.clone();
        rest.remove("unary_class");
        let mut arg = if rest.is_empty() {
            Value::Null
        } else {
            single(tx(&Value::Object(rest), depth + 1)?)
        };
        if !crate::symbols::tables()["unary_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == name)
        {
            arg = unfence(arg)
        }
        let mut model = node(class, json!({"parameter_one":arg}));
        let kind = class.rsplit("::").next().unwrap_or("");
        if ["Hat", "Bar", "Vec", "Ddot", "Dot", "Ul"].contains(&kind) {
            model["fields"]["attributes"] = json!({});
        }
        if !crate::writer::unary_class(&crate::model::Model::from_value(&model)?)
            && ![
                "Sqrt", "Abs", "Hat", "Bar", "Vec", "Ddot", "Dot", "Ul", "Cancel", "Ceil",
            ]
            .contains(&kind)
        {
            return Err(format!("unported promised source unary constructor {class}").into());
        }
        return Ok(Some(vec![model]));
    }
    Ok(None)
}
fn tx_rule_19(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("number") && m.len() != 1 {
        if m.len() == 2 && m.contains_key("sequence") {
            let mut out = tx(&m["sequence"], depth + 1)?;
            let text = m["number"]
                .as_str()
                .ok_or("number capture is not source text")?;
            out.push(number(text));
            return Ok(Some(out));
        }
        return Err("unported source combined number capture".into());
    }
    Ok(None)
}
fn tx_rule_20(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("symbol") && m.len() != 1 {
        if m.keys()
            .all(|key| ["sequence", "symbol", "expr"].contains(&key.as_str()))
        {
            let mut out = match m.get("sequence") {
                Some(value) => tx(value, depth + 1)?,
                None => Vec::new(),
            };
            let name = m["symbol"]
                .as_str()
                .ok_or("symbol capture is not source text")?;
            out.push(source_symbol(name));
            if let Some(value) = m.get("expr") {
                out.extend(tx(value, depth + 1)?);
            }
            return Ok(Some(out));
        }
        return Err("unported source combined symbol capture".into());
    }
    Ok(None)
}
fn tx_rule_21(
    m: &Map<String, Value>,
    _depth: usize,
) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(n) = m.get("number") {
        return Ok(Some(vec![number(
            n.as_str().ok_or("number capture is not source text")?,
        )]));
    }
    Ok(None)
}
fn tx_rule_22(
    m: &Map<String, Value>,
    _depth: usize,
) -> Result<Option<Vec<Value>>, ConversionError> {
    if let Some(s) = m.get("symbol") {
        return Ok(Some(vec![source_symbol(
            s.as_str().ok_or("symbol capture is not source text")?,
        )]));
    }
    Ok(None)
}
fn tx_rule_23(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("numerator") && m.contains_key("denominator") {
        let a = unfence(single(tx(&m["numerator"], depth + 1)?));
        let b = unfence(single(tx(&m["denominator"], depth + 1)?));
        return Ok(Some(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":a,"parameter_two":b}),
        )]));
    }
    Ok(None)
}
fn tx_rule_24(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.get("binary_class").and_then(Value::as_str) == Some("frac") {
        let a = unfence(single(tx(&m["base_value"], depth + 1)?));
        let b = unfence(single(tx(&m["power_value"], depth + 1)?));
        return Ok(Some(vec![node(
            "Math::Function::Frac",
            json!({"parameter_one":a,"parameter_two":b}),
        )]));
    }
    Ok(None)
}
fn tx_rule_25(m: &Map<String, Value>, depth: usize) -> Result<Option<Vec<Value>>, ConversionError> {
    if m.contains_key("lparen") && m.contains_key("rparen") {
        let a = source_symbol(m["lparen"].as_str().ok_or("fence open must be text")?);
        let b = if m["rparen"].is_null() {
            Value::Null
        } else {
            source_symbol(m["rparen"].as_str().ok_or("fence close must be text")?)
        };
        let inner = m
            .get("expr")
            .or_else(|| m.get("rgb_color"))
            .ok_or("source fence has no body capture")?;
        // Source Rules distinguish an empty Slice (nil) from an empty sequence ([]).
        let body = if inner == "" {
            Value::Null
        } else {
            Value::Array(tx(inner, depth + 1)?)
        };
        return Ok(Some(vec![node(
            "Math::Function::Fenced",
            json!({"options":{},"parameter_one":a,"parameter_two":body,"parameter_three":b}),
        )]));
    }
    Ok(None)
}
fn tx(v: &Value, depth: usize) -> Result<Vec<Value>, ConversionError> {
    let (mut v, mut depth) = (v, depth);
    // These source capture wrappers preserve their child's transformed value.
    // Iterate them instead of retaining a Rust frame for every grammar wrapper.
    while let Some(fields) = v.as_object() {
        if fields.len() != 1 {
            break;
        }
        let (key, child) = fields.iter().next().unwrap();
        if ![
            "expr",
            "sequence",
            "frac",
            "power_base",
            "base_value",
            "power_value",
            "unary",
            "intermediate_exp",
            "left_right",
            "ternary",
            "mod",
            "table",
            "comma_separated",
        ]
        .contains(&key.as_str())
        {
            break;
        }
        v = child;
        depth += 1;
        if depth > 1024 {
            return Err(ConversionError::Resource {
                depth,
                max_depth: 1024,
            });
        }
    }

    if depth > 1024 {
        return Err(ConversionError::Resource {
            depth,
            max_depth: 1024,
        });
    }
    if let Some(values) = v.as_array() {
        let mut out = vec![];
        for value in values {
            out.extend(tx(value, depth + 1)?);
        }
        return Ok(out);
    }
    if v.as_object()
        .is_some_and(|fields| fields.len() == 1 && fields.contains_key("slash"))
    {
        return Ok(vec![node(
            "Math::Function::Linebreak",
            json!({"attributes":{},"parameter_one":null}),
        )]);
    }
    if v.is_null() || v == "" {
        return Ok(vec![]);
    }
    let m = v
        .as_object()
        .ok_or_else(|| format!("unported stage capture value {v}"))?;
    if let Some(out) = tx_rule_0(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_1(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_2(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_3(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_4(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_5(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_6(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_7(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_8(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_9(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_10(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_11(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_12(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_13(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_14(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_15(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_16(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_17(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_18(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_19(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_20(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_21(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_22(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_23(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_24(m, depth)? {
        return Ok(out);
    }
    if let Some(out) = tx_rule_25(m, depth)? {
        return Ok(out);
    }
    // Fenced operands are unwrapped only at explicit fraction argument positions in source;
    // general grouping remains an unported case until its full source node is available.
    let mut out = vec![];
    for (k, x) in m {
        match k.as_str() {
            "expr" | "sequence" | "frac" | "power_base" | "base_value" | "power_value"
            | "unary" => out.extend(tx(x, depth + 1)?),
            "intermediate_exp" | "left_right" | "ternary" | "mod" | "table" => {
                out.extend(tx(x, depth + 1)?)
            }
            _ => return Err(format!("unported promised AsciiMath capture {k}").into()),
        }
    }
    Ok(out)
}
thread_local! {static CACHED_PARSER:Result<amath::Parser,String>=amath::Parser::new().map_err(|e|e.to_string());}
#[derive(Debug)]
pub enum ParseError {
    Unported(String),
    InvalidOptions(String),
    SourceOptions(Value),
    Stage(amath::Error),
    Initialization(String),
    ConverterResource { depth: usize, max_depth: usize },
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unported(s) | Self::InvalidOptions(s) | Self::Initialization(s) => f.write_str(s),
            Self::SourceOptions(error) => f.write_str(
                error["message"]
                    .as_str()
                    .unwrap_or("invalid source parse options"),
            ),
            Self::Stage(e) => e.fmt(f),
            Self::ConverterResource { depth, max_depth } => {
                write!(f, "common converter depth {depth} exceeds {max_depth}")
            }
        }
    }
}
impl std::error::Error for ParseError {}
impl From<ParseError> for String {
    fn from(e: ParseError) -> String {
        e.to_string()
    }
}
// Ruby Asciimath::Parser#legacy_text performs these replacements in order.
// The first alternation is one non-overlapping pass; sequential replacements
// of its two arms would change overlapping text such as ":|:".
fn legacy_text(input: &str) -> String {
    let mut bars = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if matches!(
            (character, chars.peek().copied()),
            ('|', Some(':')) | (':', Some('|'))
        ) {
            chars.next();
            bars.push('|');
        } else {
            bars.push(character);
        }
    }
    bars.replace("{:", "ℒ")
        .replace(":}", "ℛ")
        .replace("(:", "ᑕ")
        .replace(":)", "ᑐ")
}
pub fn parse(input: &str, format: &str, options: &Value) -> Result<Value, ParseError> {
    if format != "asciimath" {
        return Err(ParseError::Unported(format!(
            "unported promised input format {format}"
        )));
    }
    let normalized = crate::parse_options::normalize("asciimath", Some(options))
        .map_err(ParseError::SourceOptions)?;
    let decimal = crate::parse_options::decimal(&normalized);
    // Stage diagnostics intentionally address legacy_text, followed by any
    // stage preprocessing, in UTF8 bytes. Keep input_string as caller text.
    let legacy_input = legacy_text(input);
    let stage = CACHED_PARSER.with(|p| match p {
        Ok(parser) => parser
            .parse_legacy_with_decimal(&legacy_input, decimal)
            .map_err(ParseError::Stage),
        Err(message) => Err(ParseError::Initialization(message.clone())),
    })?;
    let tree =
        source_capture_tree(&stage.tree, &stage.parsed_input).map_err(ParseError::Unported)?;
    let mut values = tx(&tree, 0).map_err(|error| match error {
        ConversionError::Unported(message) => ParseError::Unported(message),
        ConversionError::Resource { depth, max_depth } => {
            ParseError::ConverterResource { depth, max_depth }
        }
    })?;
    let mut model = if values.len() == 1 && values[0]["class"] == "Math::Formula" {
        values.remove(0)
    } else {
        formula(values)
    };
    model["fields"]["input_string"] = json!(input);
    Ok(model)
}
pub fn response(v: &Value) -> Value {
    let Some(input) = v["input"].as_str() else {
        return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
    };
    let Some(format) = v["format"].as_str() else {
        return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"format must be a string"}});
    };
    match parse(input, format, v.get("options").unwrap_or(&json!({}))) {
        Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
        Err(e) => {
            let error = match &e {
                ParseError::SourceOptions(error) => error.clone(),
                ParseError::Unported(_) => {
                    json!({"code":"unported_capability","message":e.to_string()})
                }
                ParseError::InvalidOptions(_) => {
                    json!({"code":"invalid_options","message":e.to_string()})
                }
                ParseError::Initialization(_) => {
                    json!({"code":"parser_initialization_error","message":e.to_string()})
                }
                ParseError::ConverterResource { depth, max_depth } => {
                    json!({"code":"resource_limit","message":e.to_string(),"resource":{"kind":"common_model_depth","depth":depth,"max_depth":max_depth}})
                }
                ParseError::Stage(stage) => {
                    let resource = match stage {
                        amath::Error::Runtime(parsanol::PargError::RecursionLimitExceeded {
                            depth,
                            max_depth,
                        }) => Some(
                            json!({"kind":"recursion_limit","depth":depth,"max_depth":max_depth}),
                        ),
                        amath::Error::Runtime(parsanol::PargError::CallDepthLimitExceeded {
                            depth,
                            max_depth,
                        }) => Some(
                            json!({"kind":"call_depth_limit","depth":depth,"max_depth":max_depth}),
                        ),
                        _ => None,
                    };
                    json!({"code":if resource.is_some(){"resource_limit"}else if stage.byte_offset().is_some(){"syntax_error"}else{"parser_error"},"message":e.to_string(),"stage":"amath","byte_offset":stage.byte_offset(),"offset_policy":"preprocessed_utf8_bytes","native_detail":format!("{stage:?}"),"resource":resource})
                }
            };
            json!({"abi_version":1,"ok":false,"error":error})
        }
    }
}

#[cfg(test)]
mod legacy_text_source_tests {
    use super::{legacy_text, source_symbol};
    use serde_json::Value;
    #[test]
    fn ruby_legacy_delimiter_symbol_identities() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/ascii-source-closure/delimiters.json"
        ))
        .unwrap();
        for case in fixture["rows"].as_array().unwrap() {
            assert_eq!(
                source_symbol(case["text"].as_str().unwrap()),
                case["model"],
                "{}",
                case["text"]
            );
        }
    }
    #[test]
    fn ruby_legacy_text_order_and_overlaps() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/ascii-source-closure/normalization.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(
                legacy_text(case["input"].as_str().unwrap()),
                case["expected"].as_str().unwrap(),
                "{}",
                case["input"]
            );
        }
    }
}
