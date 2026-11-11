//! XML-to-common-model conversion over the accepted Moxml document parser.
//! Constructor and token tables come from the source MathML/OMML translators.
use crate::model::{formula, node, number};
use moxml::document::{ContentRow, Document, ElementRow, ParserPolicy};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;
#[path = "xml_parse/entity_wrapper.rs"]
mod entity_wrapper;

const MATHML: &str = "http://www.w3.org/1998/Math/MathML";
const OMML: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("xml_parse/source-tables.json"))
            .expect("source XML tables")
    })
}
fn error(message: impl ToString) -> Value {
    json!({"code":"INCOMPLETE_XML_TRANSLATION","message":message.to_string(),"byte_offset":0,"offset_policy":"source-xml-parser-zero"})
}
fn function(name: &str, mut values: Vec<Value>) -> Value {
    let class = format!("Math::Function::{name}");
    let family = tables()["MATHML_FUNCTIONS"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[0] == class)
        .and_then(|r| r[1].as_str())
        .unwrap_or("none");
    let arity = match family {
        "unary" => 1,
        "binary" => 2,
        "ternary" => 3,
        _ => values.len(),
    };
    values.resize(values.len().max(arity), Value::Null);
    let names = [
        "parameter_one",
        "parameter_two",
        "parameter_three",
        "parameter_four",
    ];
    let mut fields: Map<String, Value> = names
        .into_iter()
        .zip(values)
        .map(|(k, v)| (k.into(), v))
        .collect();
    if name == "Underset" {
        fields.insert("options".into(), json!({}));
    }
    node(&class, Value::Object(fields))
}
fn family(value: &Value) -> &str {
    tables()["MATHML_FUNCTIONS"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[0] == value["class"])
        .and_then(|r| r[1].as_str())
        .unwrap_or("none")
}
fn contextual(value: &str, omml: bool) -> Result<Option<Value>, Value> {
    let decoded=entity_wrapper::decode_html(value).map_err(|message|json!({"code":"PARSE_ERROR","message":message,"byte_offset":0,"offset_policy":"source-xml-parser-zero"}))?;
    let encoded = entity(&decoded);
    let key = if omml {
        "OMML_CONTEXTUAL_ACCENT_FUNCTIONS"
    } else {
        "MATHML_CONTEXTUAL_ACCENT_FUNCTIONS"
    };
    tables()[key][encoded]
        .as_str()
        .map(|name| default_function(name, omml))
        .transpose()
}

fn text(value: &str) -> Value {
    node(
        "Math::Function::Text",
        json!({"parameter_one":value,"lang":null}),
    )
}
fn omml_text(value: &str) -> Value {
    node(
        "Math::Function::Text",
        json!({"parameter_one":value,"lang":"omml"}),
    )
}
fn kind(value: &Value) -> &str {
    value["class"]
        .as_str()
        .unwrap_or("")
        .rsplit("::")
        .next()
        .unwrap_or("")
}
fn wrap(mut values: Vec<Value>) -> Value {
    values.retain(|v| !v.is_null());
    match values.len() {
        0 => Value::Null,
        1 => values.pop().unwrap(),
        _ => formula(values),
    }
}
fn is_formula(value: &Value) -> bool {
    value["class"]
        .as_str()
        .is_some_and(|s| s.starts_with("Math::Formula"))
}
fn unwrap(value: Value) -> Value {
    if is_formula(&value) {
        if let Some(items) = value["fields"]["value"].as_array() {
            if items.len() == 1 {
                return items[0].clone();
            }
        }
    }
    value
}
fn entity(value: &str) -> String {
    let mut encoded = String::new();
    for c in value.chars() {
        if !(' '..='~').contains(&c) || "<>'\"&".contains(c) {
            encoded.push_str(&format!("&#x{:x};", c as u32));
        } else {
            encoded.push(c);
        }
    }
    encoded
}
fn default_function(name: &str, omml: bool) -> Result<Value, Value> {
    let row = tables()[if omml {
        "OMML_GET_CLASS"
    } else {
        "MATHML_GET_CLASS"
    }]
    .as_array()
    .unwrap()
    .iter()
    .find(|row| row[0] == name)
    .ok_or_else(|| error(format!("unported source function {name}")))?;
    let class = row[1]
        .as_str()
        .ok_or_else(|| error(format!("source function {name} has no model class")))?;
    let mut fields = Map::new();
    for pair in row[2].as_array().unwrap() {
        let value = match pair[1].as_str() {
            Some("[]") => json!([]),
            Some("{}") => json!({}),
            _ => pair[1].clone(),
        };
        fields.insert(pair[0].as_str().unwrap().into(), value);
    }
    Ok(node(class, Value::Object(fields)))
}
fn resolve(value: &str, omml: bool) -> Result<Value, Value> {
    let encoded = entity(value);
    let mapped = tables()[if omml {
        "OMML_UNICODE_SYMBOLS"
    } else {
        "MATHML_UNICODE_SYMBOLS"
    }][encoded.trim()]
    .as_str();
    let names = tables()[if omml {
        "OMML_CLASSES"
    } else {
        "MATHML_CLASSES"
    }]
    .as_array()
    .unwrap();
    let name = mapped
        .filter(|name| names.iter().any(|v| v == name))
        .or_else(|| {
            tables()[if omml {
                "OMML_NAMED_FUNCTION_WORDS"
            } else {
                "MATHML_NAMED_FUNCTION_WORDS"
            }]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == value.trim())
            .then_some(value.trim())
        });
    if let Some(name) = name {
        return default_function(name, omml);
    }
    let key = if omml {
        "OMML_SYMBOL_CLASS_INPUT"
    } else {
        "MATHML_SYMBOL_CLASS_INPUT"
    };
    if let Some(id) = tables()[key][encoded.trim()].as_str() {
        return Ok(node(&format!("Math::Symbols::{id}"), json!({"value":null})));
    }
    Ok(node("Math::Symbols::Symbol", json!({"value":encoded})))
}
fn font(value: Value, variant: &str) -> Value {
    if let Some(class) = tables()["MATHML_FONT_STYLES"][variant].as_str() {
        node(
            class,
            json!({"parameter_one":value,"parameter_two":variant}),
        )
    } else {
        value
    }
}
fn symbol_id(value: &Value) -> &str {
    value["class"]
        .as_str()
        .unwrap_or("")
        .strip_prefix("Math::Symbols::")
        .unwrap_or("")
}
fn is_paren(value: &Value) -> bool {
    !tables()["MATHML_PAREN_SYMBOLS"][symbol_id(value)].is_null()
}
fn nary_symbol(value: &Value) -> bool {
    tables()["MATHML_NARY_SYMBOL_IDS"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == symbol_id(value))
}
fn all_parameters_empty(value: &Value) -> bool {
    ["parameter_one", "parameter_two", "parameter_three"]
        .iter()
        .all(|key| value["fields"][*key].is_null())
}
fn with_nary(symbol: Value, sub: Value, sup: Value, body: Value, style: &str) -> Value {
    let mut result = function("Nary", vec![symbol, sub, sup, body]);
    result["fields"]["options"] = json!({"type":style});
    result
}
fn nary_pair(values: Vec<Value>) -> Vec<Value> {
    if values.len() != 2 {
        return values;
    }
    let first = &values[0];
    let fields = &first["fields"];
    let body = values[1].clone();
    let nary = match kind(first) {
        "PowerBase" if nary_symbol(&fields["parameter_one"]) => Some(with_nary(
            fields["parameter_one"].clone(),
            fields["parameter_two"].clone(),
            fields["parameter_three"].clone(),
            body,
            "subSup",
        )),
        "Power" if nary_symbol(&fields["parameter_one"]) => Some(with_nary(
            fields["parameter_one"].clone(),
            Value::Null,
            fields["parameter_two"].clone(),
            body,
            "subSup",
        )),
        "Overset" if nary_symbol(&fields["parameter_two"]) => Some(with_nary(
            fields["parameter_two"].clone(),
            Value::Null,
            fields["parameter_one"].clone(),
            body,
            "undOvr",
        )),
        _ => None,
    };
    nary.map_or(values, |v| vec![v])
}
fn organize_row(mut values: Vec<Value>) -> Value {
    if values.len() > 1
        && kind(&values[0]) == "Underset"
        && nary_symbol(&values[0]["fields"]["parameter_two"])
    {
        let first = values.remove(0);
        return with_nary(
            first["fields"]["parameter_two"].clone(),
            first["fields"]["parameter_one"].clone(),
            Value::Null,
            wrap(values),
            "undOvr",
        );
    }

    values = nary_pair(values);
    let mut index = 0;
    while index < values.len() {
        if index + 1 < values.len() {
            if kind(&values[index]) == "Nary"
                && values[index]["fields"]["parameter_four"].is_null()
                && !matches!(
                    kind(&unwrap(values[index + 1].clone())),
                    "Base" | "PowerBase"
                )
            {
                let body = values.remove(index + 1);
                values[index]["fields"]["parameter_four"] = body;
                index += 1;
                continue;
            }
            if kind(&values[index + 1]) == "Mod" {
                let first = values.remove(index);
                let mut modulo = values.remove(index);
                let second = if index < values.len() {
                    values.remove(index)
                } else {
                    Value::Null
                };
                modulo["fields"]["parameter_one"] = unwrap(first);
                modulo["fields"]["parameter_two"] = unwrap(second);
                values.insert(index, modulo);
                index += 1;
                continue;
            }
            let class = values[index]["class"].as_str().unwrap_or("");
            let name = tables()["MATHML_FUNCTIONS"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r[0] == class)
                .and_then(|r| r[2].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| kind(&values[index]).to_ascii_lowercase());
            let updatable = family(&values[index]) == "unary"
                && tables()["MATHML_UNARY_CLASSES"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n == &name);
            if updatable {
                let next = values.remove(index + 1);
                values[index]["fields"]["parameter_one"] = unwrap(next);
            } else if family(&values[index]) == "ternary"
                && !all_parameters_empty(&values[index])
                && values[index]["fields"]["parameter_three"].is_null()
            {
                let next = values.remove(index + 1);
                values[index]["fields"]["parameter_three"] = next;
            }
        }
        index += 1;
    }
    if values.len() >= 2 && is_paren(&values[0]) && is_paren(values.last().unwrap()) {
        let open = values.remove(0);
        let close = values.pop().unwrap();
        if values.len() == 1 && kind(&values[0]) == "Table" {
            let mut table = values.remove(0);
            table["fields"]["open_paren"] = open;
            table["fields"]["close_paren"] = close;
            return table;
        }
        let mut result = function("Fenced", vec![open, json!(values), close]);
        result["fields"]["options"] = json!({});
        return result;
    }
    wrap(values)
}
fn math_text(value: &str) -> Result<Value, Value> {
    let decoded=entity_wrapper::decode_html(value).map_err(|message|json!({"code":"PARSE_ERROR","message":message,"byte_offset":0,"offset_policy":"source-xml-parser-zero"}))?;
    let mut encoded = entity(&decoded);
    for (code, name) in tables()["MATHML_UNICODE_SYMBOLS"].as_object().unwrap() {
        encoded = encoded.replace(
            &code.to_lowercase(),
            &format!("unicode[:{}]", name.as_str().unwrap()),
        );
    }
    Ok(text(&encoded))
}
fn omml_text_symbol(value: &str) -> Result<Value, Value> {
    let resolved = resolve(value, true)?;
    if resolved["class"]
        .as_str()
        .is_some_and(|name| name.starts_with("Math::Function::"))
    {
        return Ok(resolved);
    }
    let mut encoded = entity(value);
    for (reference, character) in [
        ("&#x26;", "&"),
        ("&#x3c;", "<"),
        ("&#x27;", "'"),
        ("&#xa0;", "\u{a0}"),
        ("&#x3e;", ">"),
        ("&#xa;", "\n"),
    ] {
        encoded = encoded.replace(reference, character);
    }
    let stripped =
        encoded.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\0'));
    Ok(
        if let Some(id) = tables()["OMML_SYMBOL_CLASS_INPUT"][stripped].as_str() {
            node(&format!("Math::Symbols::{id}"), json!({"value":null}))
        } else {
            node("Math::Symbols::Symbol", json!({"value":encoded}))
        },
    )
}
fn omml_font(value: Value, variant: &str) -> Value {
    let Some(class) = tables()["OMML_FONT_STYLES"][variant].as_str() else {
        return value;
    };
    let keyword = tables()["OMML_FONT_STYLES"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, v)| v.as_str() == Some(class))
        .map(|(k, _)| k.as_str())
        .unwrap();
    node(
        class,
        json!({"parameter_one":value,"parameter_two":keyword}),
    )
}
struct Xml {
    rows: Vec<ElementRow>,
}
impl Xml {
    fn row(&self, id: usize) -> &ElementRow {
        &self.rows[id]
    }
    fn ns(&self, id: usize) -> &str {
        self.row(id).name.namespace.as_deref().unwrap_or("")
    }
    fn name(&self, id: usize) -> &str {
        &self.row(id).name.local
    }
    fn attr(&self, id: usize, name: &str) -> Option<&str> {
        let namespace = if self.ns(id).is_empty() || self.ns(id) == MATHML {
            ""
        } else {
            self.ns(id)
        };
        self.row(id)
            .attributes
            .iter()
            .find(|a| {
                a.name.local == name && a.name.namespace.as_deref().unwrap_or("") == namespace
            })
            .map(|a| a.value.as_str())
    }
    fn children(&self, id: usize) -> Vec<usize> {
        self.row(id)
            .content
            .iter()
            .filter_map(|c| {
                if let ContentRow::Element(i) = c {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect()
    }
    fn child(&self, id: usize, name: &str) -> Option<usize> {
        self.children(id)
            .into_iter()
            .find(|i| self.name(*i) == name && self.ns(*i) == OMML)
    }
    fn val(&self, id: Option<usize>) -> Option<&str> {
        id.and_then(|id| self.attr(id, "val"))
    }
    fn direct_text(&self, id: usize) -> String {
        self.row(id)
            .content
            .iter()
            .filter_map(|c| match c {
                ContentRow::Text(s) | ContentRow::CData(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }
    fn math_positions(&self, id: usize) -> Result<Vec<Value>, Value> {
        let mut values = Vec::new();
        for child in &self.row(id).content {
            match child {
                ContentRow::Element(i)
                    if (self.ns(*i).is_empty() || self.ns(*i) == MATHML)
                        && !matches!(self.name(*i), "malignmark" | "maligngroup" | "math") =>
                {
                    values.push(self.math(*i)?)
                }
                ContentRow::Text(v) | ContentRow::CData(v)
                    if !v
                        .trim_matches(|c| {
                            matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\0')
                        })
                        .is_empty() =>
                {
                    values.push(if v.chars().all(char::is_whitespace) {
                        Value::Null
                    } else {
                        resolve(v, false)?
                    })
                }
                _ => {}
            }
        }
        Ok(values)
    }
    fn math_children(&self, id: usize) -> Result<Vec<Value>, Value> {
        let mut values = Vec::new();
        for child in &self.row(id).content {
            let value = match child {
                ContentRow::Element(i)
                    if (self.ns(*i).is_empty() || self.ns(*i) == MATHML)
                        && self.name(*i) != "math" =>
                {
                    self.math(*i)?
                }
                ContentRow::Text(s) | ContentRow::CData(s) if !s.trim().is_empty() => {
                    resolve(s, false)?
                }
                _ => Value::Null,
            };
            if !value.is_null() {
                values.push(value);
            }
        }
        Ok(values)
    }
    fn math(&self, id: usize) -> Result<Value, Value> {
        let name = self.name(id);
        if matches!(name, "mi" | "mo" | "mn" | "mtext" | "ms") {
            let value = self.direct_text(id);
            return match name {
                "mn" => Ok(number(&value)),
                "mtext" => math_text(&value),
                "ms" => {
                    let value = self.ms_text(id);
                    Ok(function(
                        "Ms",
                        vec![if value.is_empty() {
                            Value::Null
                        } else {
                            json!(value)
                        }],
                    ))
                }
                "mi" if value.is_empty() => Ok(Value::Null),
                _ => {
                    let mut token = if value.is_empty() {
                        node("Math::Symbols::Symbol", json!({"value":null}))
                    } else {
                        resolve(&value, false)?
                    };
                    if name == "mo" {
                        if let Some(space) = self.attr(id, "rspace") {
                            if kind(&token) == "Symbol" {
                                token["fields"]["options"] = json!({"rspace":space});
                            }
                        }
                        if self.attr(id, "linebreak") == Some("newline") {
                            let mut attrs = Map::new();
                            if let Some(style) = self.attr(id, "linebreakstyle") {
                                attrs.insert("linebreakstyle".into(), json!(style));
                            }
                            return Ok(node(
                                "Math::Function::Linebreak",
                                json!({"parameter_one":token,"attributes":attrs}),
                            ));
                        }
                    }
                    Ok(font(token, self.attr(id, "mathvariant").unwrap_or("")))
                }
            };
        }
        if matches!(
            name,
            "none"
                | "mprescripts"
                | "malignmark"
                | "maligngroup"
                | "annotation"
                | "annotation-xml"
                | "mscarry"
        ) {
            return Ok(Value::Null);
        }
        if name == "mspace" {
            return Ok(match self.attr(id, "linebreak").filter(|s| !s.is_empty()) {
                Some(linebreak) => node(
                    "Math::Function::Linebreak",
                    json!({"parameter_one":null,"attributes":{"linebreak":linebreak}}),
                ),
                None => Value::Null,
            });
        }
        if name == "mmultiscripts" {
            return self.multiscripts(id);
        }
        let mut values = if matches!(
            name,
            "mfrac"
                | "mfraction"
                | "msup"
                | "msub"
                | "msubsup"
                | "mroot"
                | "mover"
                | "munder"
                | "munderover"
        ) {
            self.math_positions(id)?
        } else {
            self.math_children(id)?
        };
        let at = |index: usize| values.get(index).cloned().unwrap_or(Value::Null);
        let value = match name {
            "math" => {
                values = nary_pair(values);
                let mut display = self.attr(id, "display") != Some("false");
                if values.len() == 1 && kind(&values[0]) == "Mstyle" {
                    display = values[0]["fields"]["displaystyle"]
                        .as_bool()
                        .unwrap_or(true);
                    values = values[0]["fields"]["value"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                }
                let mut result = formula(values);
                result["fields"]["displaystyle"] = json!(display);
                result
            }
            "mrow" => {
                let result = organize_row(values);
                if let Some(intent) = self.attr(id, "intent").filter(|s| !s.is_empty()) {
                    if intent.starts_with(":") || intent.contains("uncertainty") {
                        return Err(error(
                            "MathML uncertainty intent conversion remains incomplete",
                        ));
                    }
                    if is_formula(&result)
                        && result["fields"]["value"]
                            .as_array()
                            .is_some_and(|a| a.len() > 1)
                    {
                        let mut row = result;
                        row["class"] = json!("Math::Formula::Mrow");
                        row["fields"]["is_mrow"] = json!(true);
                        function("Intent", vec![row, text(intent)])
                    } else {
                        result
                    }
                } else {
                    result
                }
            }
            "mfrac" | "mfraction" => {
                let mut result = function("Frac", vec![at(0), at(1)]);
                let mut options = Map::new();
                for name in ["linethickness", "bevelled"] {
                    if let Some(value) = self.attr(id, name) {
                        options.insert(name.into(), json!(value));
                    }
                }
                if !options.is_empty() {
                    result["fields"]["options"] = json!(options);
                }
                result
            }
            "msup" | "msub" => {
                let mut base = unwrap(at(0));
                let script = unwrap(at(1));
                if family(&base) == "binary" && all_parameters_empty(&base) {
                    base["fields"]["parameter_one"] = script;
                    base
                } else {
                    function(
                        if name == "msup" { "Power" } else { "Base" },
                        vec![base, script],
                    )
                }
            }
            "msubsup" => {
                let mut base = unwrap(at(0));
                let sub = unwrap(at(1));
                let sup = unwrap(at(2));
                if matches!(family(&base), "binary" | "ternary") && all_parameters_empty(&base) {
                    base["fields"]["parameter_one"] = sub;
                    base["fields"]["parameter_two"] = sup;
                    base
                } else {
                    function("PowerBase", vec![base, sub, sup])
                }
            }
            "msqrt" => function("Sqrt", vec![wrap(values)]),
            "mroot" => function("Root", vec![unwrap(at(1)), unwrap(at(0))]),
            "mover" | "munder" => self.math_accent(id, &values)?,
            "munderover" => {
                let mut base = unwrap(at(0));
                let under = unwrap(at(1));
                let over = unwrap(at(2));
                if family(&base) == "ternary" && all_parameters_empty(&base) {
                    base["fields"]["parameter_one"] = under;
                    base["fields"]["parameter_two"] = over;
                    base
                } else if nary_symbol(&base) {
                    with_nary(base, under, over, Value::Null, "undOvr")
                } else {
                    function("Underover", vec![base, under, over])
                }
            }
            "mphantom" => function("Phantom", vec![wrap(values)]),
            "merror" => function("Merror", vec![wrap(values)]),
            "mstack" => function("Stackrel", vec![wrap(values)]),
            "msrow" => {
                if values.is_empty() {
                    Value::Null
                } else {
                    formula(values)
                }
            }
            "msline" => function("Msline", vec![wrap(values)]),
            "msgroup" => {
                let mut content = Vec::new();
                for item in &self.row(id).content {
                    let value = match item {
                        ContentRow::Text(v) | ContentRow::CData(v)
                            if !v
                                .trim_matches(|c| {
                                    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\0')
                                })
                                .is_empty() =>
                        {
                            text(v)
                        }
                        ContentRow::Element(i) => self.math(*i)?,
                        _ => Value::Null,
                    };
                    if !value.is_null() {
                        content.push(value);
                    }
                }
                function("Msgroup", vec![json!(content)])
            }
            "mlongdiv" => function("Longdiv", vec![json!(values)]),
            "mscarries" => function("Scarries", vec![wrap(values)]),
            "menclose" => function(
                "Menclose",
                vec![
                    self.attr(id, "notation").map_or(Value::Null, |v| json!(v)),
                    if values.len() == 1 {
                        at(0)
                    } else {
                        formula(values)
                    },
                ],
            ),
            "mpadded" => {
                let mut result = function("Mpadded", vec![wrap(values)]);
                let mut options = Map::new();
                for key in ["height", "depth", "width"] {
                    if let Some(v) = self.attr(id, key) {
                        options.insert(key.into(), json!(v));
                    }
                }
                if !options.is_empty() {
                    result["fields"]["options"] = json!(options);
                }
                result
            }
            "mstyle" => {
                let color = self.attr(id, "mathcolor").filter(|s| !s.is_empty());
                let variant = self.attr(id, "mathvariant").unwrap_or("");
                let content = wrap(values.clone());
                let content = if let Some(color) = color {
                    function("Color", vec![text(color), content])
                } else {
                    content
                };
                if tables()["MATHML_FONT_STYLES"][variant].is_string() {
                    font(content, variant)
                } else if color.is_some() {
                    content
                } else {
                    node(
                        "Math::Formula::Mstyle",
                        json!({"value":values,"left_right_wrapper":true,"displaystyle":self.attr(id,"displaystyle")!=Some("false")}),
                    )
                }
            }
            "mfenced" => {
                let open = self.attr(id, "open");
                let close = self.attr(id, "close");
                let left = open.or_else(|| if close.is_none() { Some("(") } else { None });
                let right = close.or_else(|| if open.is_none() { Some(")") } else { None });
                let mut result = function(
                    "Fenced",
                    vec![
                        left.map(|s| resolve(s, false))
                            .transpose()?
                            .unwrap_or(Value::Null),
                        json!(values.into_iter().map(unwrap).collect::<Vec<_>>()),
                        right
                            .map(|s| resolve(s, false))
                            .transpose()?
                            .unwrap_or(Value::Null),
                    ],
                );
                result["fields"]["options"] = self
                    .attr(id, "separators")
                    .map_or(json!({}), |s| json!({"separators":s}));
                result
            }
            "mtd" => function("Td", vec![json!(values), Value::Null]),
            "mtr" => function("Tr", vec![json!(values)]),
            "mtable" => {
                let mut options = Map::new();
                for key in ["frame", "rowlines"] {
                    if let Some(v) = self.attr(id, key).filter(|s| !s.is_empty()) {
                        options.insert(key.into(), json!(v));
                    }
                }
                if let Some(lines) = self.attr(id, "columnlines").filter(|s| !s.is_empty()) {
                    let parts = lines.split_ascii_whitespace().collect::<Vec<_>>();
                    if !parts.iter().all(|s| s.contains("none")) {
                        let separator = function(
                            "Td",
                            vec![
                                json!([node("Math::Symbols::Paren::Vert", json!({"value":null}))]),
                                Value::Null,
                            ],
                        );
                        for (index, part) in parts.iter().enumerate() {
                            if *part == "solid" {
                                for row in &mut values {
                                    let cells =
                                        row["fields"]["parameter_one"].as_array_mut().ok_or_else(
                                            || error("MathML columnlines row has no cells"),
                                        )?;
                                    while cells.len() < index + 1 {
                                        cells.push(Value::Null);
                                    }
                                    cells.insert(index + 1, separator.clone());
                                    if let Some(i) = cells.iter().position(Value::is_null) {
                                        cells[i] = function("Td", vec![json!([]), Value::Null]);
                                    }
                                }
                            }
                        }
                        options.insert("columnlines".into(), json!(lines));
                    }
                }
                node(
                    "Math::Function::Table",
                    json!({"value":values,"open_paren":null,"close_paren":null,"options":options}),
                )
            }
            "mlabeledtr" => function(
                "Mlabeledtr",
                vec![json!(values), self.attr(id, "id").map_or(Value::Null, text)],
            ),
            "semantics" => {
                let mut annotations = Vec::new();
                for tag in ["annotation", "annotation-xml"] {
                    for child in self.children(id) {
                        if self.name(child) == tag {
                            if tag == "annotation-xml" {
                                return Err(error(
                                    "source annotation-xml object identity is not portable",
                                ));
                            }
                            let mut entry = Map::new();
                            entry.insert(
                                tag.into(),
                                json!([node(
                                    "Math::Symbols::Symbol",
                                    json!({"value":self.direct_text(child)})
                                )]),
                            );
                            annotations.push(Value::Object(entry));
                        }
                    }
                }
                function(
                    "Semantics",
                    vec![
                        unwrap(wrap(values)),
                        if annotations.is_empty() {
                            Value::Null
                        } else {
                            json!(annotations)
                        },
                    ],
                )
            }
            "mglyph" => function(
                "Mglyph",
                vec![
                    json!({"src":self.attr(id,"src"),"alt":self.attr(id,"alt"),"index":self.attr(id,"index").and_then(|v|v.parse::<i64>().ok())}),
                ],
            ),
            _ => {
                return Err(error(format!(
                    "MathML element {name} conversion remains incomplete"
                )))
            }
        };
        Ok(value)
    }
    fn ms_text(&self, id: usize) -> String {
        let mut parts = Vec::new();
        for item in &self.row(id).content {
            match item {
                ContentRow::Text(v) | ContentRow::CData(v)
                    if !v
                        .trim_matches(|c| {
                            matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\0')
                        })
                        .is_empty() =>
                {
                    parts.push(v.clone())
                }
                ContentRow::Element(i) => parts.push(
                    if matches!(self.name(*i), "mi" | "mn" | "mo" | "ms" | "mtext") {
                        self.direct_text(*i)
                    } else {
                        self.ms_text(*i)
                    },
                ),
                _ => {}
            }
        }
        parts.join(" ")
    }
    fn multiscripts(&self, id: usize) -> Result<Value, Value> {
        let mut children = Vec::new();
        let mut pre = None;
        for row in self.children(id) {
            if self.name(row) == "mprescripts" {
                pre = Some(children.len());
                continue;
            }
            let value = if self.name(row) == "none"
                || matches!(self.name(row), "mi" | "mo" | "mn" | "mtext" | "ms")
                    && self.direct_text(row).is_empty()
            {
                function("None", vec![Value::Null])
            } else {
                self.math(row)?
            };
            children.push(if value.is_null() {
                function("None", vec![Value::Null])
            } else {
                value
            });
        }
        if children.is_empty() {
            return Ok(function(
                "Multiscript",
                vec![Value::Null, Value::Null, Value::Null],
            ));
        }
        let base = children.remove(0);
        let split = |items: &[Value]| {
            let mut subs = Vec::new();
            let mut sups = Vec::new();
            for pair in items.chunks(2) {
                subs.push(pair[0].clone());
                if let Some(sup) = pair.get(1) {
                    sups.push(sup.clone());
                }
            }
            (subs, sups)
        };
        let attach = |base: Value, sub: Vec<Value>, sup: Vec<Value>| {
            if sub.is_empty() && sup.is_empty() {
                unwrap(base)
            } else {
                function("PowerBase", vec![unwrap(base), wrap(sub), wrap(sup)])
            }
        };
        if let Some(pre) = pre {
            let count = pre.saturating_sub(1);
            let (post_sub, post_sup) = split(&children[..count]);
            let (pre_sub, pre_sup) = split(&children[count..]);
            Ok(function(
                "Multiscript",
                vec![
                    attach(base, post_sub, post_sup),
                    json!(pre_sub),
                    json!(pre_sup),
                ],
            ))
        } else {
            let (sub, sup) = split(&children);
            Ok(function(
                "Multiscript",
                vec![attach(base, sub, sup), Value::Null, Value::Null],
            ))
        }
    }
    fn math_accent(&self, id: usize, values: &[Value]) -> Result<Value, Value> {
        let mut base = values.first().cloned().map(unwrap).unwrap_or(Value::Null);
        let mut mark = values.get(1).cloned().map(unwrap).unwrap_or(Value::Null);
        if self.name(id) == "munder"
            && ((kind(&base) == "Vec")
                || matches!(family(&base), "binary" | "ternary") && all_parameters_empty(&base))
        {
            base["fields"]["parameter_one"] = mark;
            return Ok(base);
        }

        if kind(&mark) == "Symbol" {
            let encoded = mark["fields"]["value"].as_str().unwrap_or("");
            if let Some(name) = tables()["MATHML_CONTEXTUAL_ACCENT_FUNCTIONS"][encoded].as_str() {
                mark = default_function(name, false)?;
            }
        }
        let name = kind(&mark);
        let under = self.name(id) == "munder";
        let accent_name = match name {
            "Hat" => Some("Hat"),
            "Tilde" => Some("Tilde"),
            "Vec" => Some("Vec"),
            "Ddot" => Some("Ddot"),
            "Dot" | "Period" => Some("Dot"),
            "Bar" => Some(if under { "Ul" } else { "Bar" }),
            "Ul" | "Underline" => Some(if under { "Ul" } else { "Bar" }),
            "Obrace" => Some("Obrace"),
            "Ubrace" => Some("Ubrace"),
            _ => None,
        };
        let key = if under { "accentunder" } else { "accent" };
        let mut attrs = Map::new();
        if self.attr(id, key) == Some("true") {
            attrs.insert(key.into(), json!(true));
        }
        if let Some(name) = accent_name {
            if matches!(kind(&mark), "Obrace" | "Ubrace")
                || (!under && kind(&mark) == "Bar")
                || (under && matches!(kind(&mark), "Ul" | "Underline"))
            {
                mark["fields"]["parameter_one"] = base;
                return Ok(mark);
            }

            let mut result = function(name, vec![base]);
            result["fields"]["attributes"] = json!(attrs);
            return Ok(result);
        }
        let mut result = function(if under { "Underset" } else { "Overset" }, vec![mark, base]);
        if !attrs.is_empty() {
            result["fields"]["options"] = json!(attrs);
        }
        Ok(result)
    }
    fn argument(&self, id: Option<usize>) -> Result<Value, Value> {
        match id {
            Some(id) => Ok(wrap(self.omml_children(id)?)),
            None => Ok(Value::Null),
        }
    }
    fn omml_children(&self, id: usize) -> Result<Vec<Value>, Value> {
        let mut values = Vec::new();
        for child in self.children(id) {
            if self.ns(child) != OMML {
                continue;
            }
            let value = self.omml(child)?;
            if !value.is_null() {
                values.push(unwrap(value));
            }
        }
        Ok(values)
    }
    fn argument_values(&self, id: usize) -> Result<Vec<Value>, Value> {
        let value = self.argument(Some(id))?;
        Ok(if is_formula(&value) {
            value["fields"]["value"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        } else if value.is_null() {
            vec![]
        } else {
            vec![value]
        })
    }
    fn table(&self, rows: Vec<Value>) -> Value {
        node(
            "Math::Function::Table",
            json!({"value":rows,"open_paren":null,"close_paren":null,"options":{}}),
        )
    }
    fn omml_matrix_row(&self, id: usize) -> Result<Value, Value> {
        let mut cells = Vec::new();
        for child in self.children(id) {
            if self.ns(child) == OMML && self.name(child) == "e" {
                cells.push(function(
                    "Td",
                    vec![json!(self.argument_values(child)?), Value::Null],
                ));
            }
        }
        Ok(function("Tr", vec![json!(cells)]))
    }
    fn omml(&self, id: usize) -> Result<Value, Value> {
        let name = self.name(id);
        let arg = |name: &str| self.argument(self.child(id, name));
        match name {
            "oMathPara" => {
                let mut values = Vec::new();
                for child in self.children(id) {
                    if self.ns(child) == OMML && matches!(self.name(child), "oMath" | "r") {
                        let value = self.omml(child)?;
                        if is_formula(&value) {
                            values.extend(
                                value["fields"]["value"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default(),
                            );
                        } else if !value.is_null() {
                            values.push(value);
                        }
                    }
                }
                Ok(formula(values))
            }
            "oMath" => Ok(formula(self.omml_children(id)?)),
            "t" => {
                let value = self.direct_text(id).replace("&nbsp;", "\u{a0}");
                if value == "\u{200b}" {
                    return Ok(Value::Null);
                }
                if value.chars().any(|c| c.is_ascii_alphabetic()) {
                    if tables()["OMML_NAMED_FUNCTION_WORDS"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|s| s == value.trim())
                    {
                        resolve(&value, true)
                    } else {
                        Ok(omml_text(&value))
                    }
                } else if !value.is_empty() && value.chars().all(|c| c.is_numeric()) {
                    Ok(number(&value))
                } else if value.is_empty() {
                    Ok(omml_text(""))
                } else {
                    omml_text_symbol(&value)
                }
            }
            "r" => {
                let mut values = Vec::new();
                let mut variant = None;
                for child in self.children(id) {
                    match (self.ns(child), self.name(child)) {
                        (OMML, "t") => {
                            let value = self.omml(child)?;
                            if !value.is_null() {
                                values.push(value);
                            }
                        }
                        (WORD, "br") => values.push(node(
                            "Math::Function::Linebreak",
                            json!({"parameter_one":null,"attributes":{}}),
                        )),
                        (OMML, "rPr") if variant.is_none() && values.is_empty() => {
                            let parts = [
                                self.val(self.child(child, "scr")),
                                self.val(self.child(child, "sty")),
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join("-");
                            variant = tables()["OMML_SUPPORTED_FONTS"][parts]
                                .as_str()
                                .map(str::to_owned);
                        }
                        (WORD, "rPr") if variant.is_none() && values.is_empty() => {
                            let enabled = |name: &str| {
                                self.children(child)
                                    .into_iter()
                                    .find(|i| self.ns(*i) == WORD && self.name(*i) == name)
                                    .is_some_and(|i| {
                                        !matches!(self.attr(i, "val"), Some("0" | "false" | "off"))
                                    })
                            };
                            variant = match (enabled("b"), enabled("i")) {
                                (true, true) => Some("bold-italic".into()),
                                (true, false) => Some("bold".into()),
                                (false, true) => Some("italic".into()),
                                _ => None,
                            };
                        }
                        _ => {}
                    }
                }
                let value = wrap(values);
                Ok(if let Some(variant) = variant {
                    omml_font(value, &variant)
                } else {
                    value
                })
            }
            "f" => Ok(function("Frac", vec![arg("num")?, arg("den")?])),
            "sSup" => Ok(function("Power", vec![arg("e")?, arg("sup")?])),
            "sSub" => Ok(function("Base", vec![arg("e")?, arg("sub")?])),
            "sSubSup" => {
                let base = arg("e")?;
                let inner = &base["fields"]["parameter_one"];
                let extracted = if base["class"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("Math::Function::FontStyle"))
                {
                    if kind(inner) == "Text" {
                        inner["fields"]["parameter_one"].as_str().map(str::to_owned)
                    } else {
                        Some(kind(inner).to_ascii_lowercase())
                    }
                } else {
                    None
                };
                if let Some(name) = extracted {
                    let valid = tables()["OMML_POWER_BASE_NAMES"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v == &name)
                        || kind(inner) == "Text"
                            && tables()["OMML_SUB_SUP_CLASSES"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|v| v == &name);
                    if valid {
                        let class = tables()["OMML_SCRIPT_CLASSES"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r[0] == name)
                            .and_then(|r| r[1].as_str())
                            .ok_or_else(|| {
                                error(format!("source OMML script class {name} is unavailable"))
                            })?;
                        return Ok(node(
                            class,
                            json!({"parameter_one":arg("sub")?,"parameter_two":arg("sup")?}),
                        ));
                    }
                }
                Ok(function("PowerBase", vec![base, arg("sub")?, arg("sup")?]))
            }
            "rad" => {
                let degree = arg("deg")?;
                let body = arg("e")?;
                Ok(if degree.is_null() {
                    function("Sqrt", vec![body])
                } else {
                    function("Root", vec![degree, body])
                })
            }
            "bar" => Ok(node(
                "Math::Function::Bar",
                json!({"parameter_one":arg("e")?,"attributes":{"accent":false}}),
            )),
            "limUpp" | "limLow" => {
                let base = arg("e")?;
                let mut lim = arg("lim")?;
                let named = tables()["OMML_NAMED_FUNCTION_WORDS"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| {
                        v.as_str()
                            .is_some_and(|n| kind(&lim).eq_ignore_ascii_case(n))
                    });
                if family(&lim) == "unary" && lim["fields"]["parameter_one"].is_null() && !named {
                    lim["fields"]["parameter_one"] = base;
                    Ok(lim)
                } else {
                    Ok(function(
                        if name == "limUpp" {
                            "Overset"
                        } else {
                            "Underset"
                        },
                        vec![lim, base],
                    ))
                }
            }
            "nary" => {
                let props = self.child(id, "naryPr");
                let chr = props.and_then(|p| self.val(self.child(p, "chr")));
                let mut symbol = chr
                    .map(|v| resolve(v, true))
                    .transpose()?
                    .unwrap_or(Value::Null);
                if family(&symbol) == "ternary" {
                    symbol["fields"]["parameter_one"] = arg("sub")?;
                    symbol["fields"]["parameter_two"] = arg("sup")?;
                    symbol["fields"]["parameter_three"] = arg("e")?;
                    Ok(symbol)
                } else {
                    let encoded = entity(chr.unwrap_or("∫"));
                    let symbol = if let Some(id) =
                        tables()["OMML_SYMBOL_CLASS_INPUT"][encoded.trim()].as_str()
                    {
                        node(&format!("Math::Symbols::{id}"), json!({"value":null}))
                    } else {
                        node("Math::Symbols::Symbol", json!({"value":encoded}))
                    };
                    let mut value =
                        function("Nary", vec![symbol, arg("sub")?, arg("sup")?, arg("e")?]);
                    value["fields"]["options"] = json!({"type":props.and_then(|p|self.val(self.child(p,"limLoc"))).unwrap_or("subSup")});
                    Ok(value)
                }
            }
            "d" => {
                let props = self.child(id, "dPr");
                let boundary = |name: &str, fallback: &str| -> Result<Value, Value> {
                    let prop = props.and_then(|p| self.child(p, name));
                    match prop.map(|p| self.attr(p, "val")) {
                        None => resolve(fallback, true),
                        Some(Some(v)) if !v.is_empty() => resolve(v, true),
                        _ => Ok(Value::Null),
                    }
                };
                let left = boundary("begChr", "(")?;
                let right = boundary("endChr", ")")?;
                let mut values = Vec::new();
                for child in self.children(id) {
                    if self.ns(child) == OMML && self.name(child) == "e" {
                        values.extend(self.argument_values(child)?);
                    }
                }
                if values.len() == 1 && kind(&values[0]) == "Table" {
                    let mut table = values.remove(0);
                    table["fields"]["open_paren"] = left;
                    table["fields"]["close_paren"] = right;
                    Ok(table)
                } else {
                    let mut value = function("Fenced", vec![left, json!(values), right]);
                    let sep = props.and_then(|p| self.child(p, "sepChr"));
                    value["fields"]["options"] =
                        sep.map_or(Value::Null, |p| json!({"sepChr":self.attr(p,"val")}));
                    Ok(value)
                }
            }
            "m" => {
                let mut rows = Vec::new();
                for child in self.children(id) {
                    if self.ns(child) == OMML && self.name(child) == "mr" {
                        rows.push(self.omml_matrix_row(child)?);
                    }
                }
                Ok(self.table(rows))
            }
            "eqArr" => {
                let mut rows = Vec::new();
                for child in self.children(id) {
                    if self.ns(child) == OMML && self.name(child) == "e" {
                        rows.push(function(
                            "Tr",
                            vec![json!([function(
                                "Td",
                                vec![json!(self.argument_values(child)?), Value::Null]
                            )])],
                        ));
                    }
                }
                Ok(self.table(rows))
            }
            "func" => {
                let mut function_name = arg("fName")?;
                let argument = arg("e")?;
                if family(&function_name) == "unary"
                    && function_name["fields"]["parameter_one"].is_null()
                {
                    function_name["fields"]["parameter_one"] = argument;
                    Ok(function_name)
                } else {
                    Ok(wrap(vec![function_name, argument]))
                }
            }
            "groupChr" => {
                let props = self.child(id, "groupChrPr");
                let chr = props.and_then(|p| self.val(self.child(p, "chr")));
                let top = props.and_then(|p| self.val(self.child(p, "pos"))) == Some("top");
                let symbol = node(
                    "Math::Symbols::Symbol",
                    json!({"value":chr.unwrap_or(if top{""}else{"⏟"})}),
                );
                Ok(function(
                    if top { "Overset" } else { "Underset" },
                    vec![symbol, arg("e")?],
                ))
            }
            "phant" | "box" => arg("e"),
            "borderBox" => Ok(function("Menclose", vec![json!("longdiv"), arg("e")?])),
            "sPre" => {
                let sub = arg("sub")?;
                let sup = arg("sup")?;
                Ok(function(
                    "Multiscript",
                    vec![
                        arg("e")?,
                        if sub.is_null() {
                            json!([])
                        } else {
                            json!([sub])
                        },
                        if sup.is_null() {
                            json!([])
                        } else {
                            json!([sup])
                        },
                    ],
                ))
            }
            "acc" => {
                let chr = self
                    .child(id, "accPr")
                    .and_then(|p| self.val(self.child(p, "chr")));
                let mut mark = match chr {
                    None => default_function("hat", true)?,
                    Some(chr) => contextual(chr, true)?.unwrap_or(resolve(chr, true)?),
                };
                let body = arg("e")?;
                if family(&mark) != "unary" {
                    let mut value = function("Overset", vec![mark, body]);
                    value["fields"]["options"] = json!({"accent":true});
                    Ok(value)
                } else {
                    if mark["fields"]["parameter_one"].is_null() {
                        mark["fields"]["parameter_one"] = body;
                    }
                    mark["fields"]["attributes"] = json!({"accent":true});
                    Ok(mark)
                }
            }
            name if name.ends_with("Pr") || name == "ctrlPr" => Ok(Value::Null),
            _ => Err(error(format!(
                "OMML element {name} conversion remains incomplete"
            ))),
        }
    }
}

pub(crate) fn source_document_rows(input: &str) -> Result<Vec<ElementRow>, Value> {
    let prepared = entity_wrapper::prepare(input);
    let doc =
        Document::parse_with(prepared.as_bytes(), &ParserPolicy::default()).map_err(|error| {
            json!({
                "code":"PARSE_ERROR",
                "message":error.to_string(),
                "byte_offset":0,
                "offset_policy":"source-xml-parser-zero"
            })
        })?;
    let mut rows = doc.element_rows();
    entity_wrapper::restore_rows(&mut rows);
    Ok(rows)
}
pub(crate) fn source_entity_decode(input: &str) -> Result<String, Value> {
    entity_wrapper::decode_html(input).map_err(|message|json!({"code":"PARSE_ERROR","message":message,"byte_offset":0,"offset_policy":"source-xml-parser-zero"}))
}

/// Parse source XML through Moxml and construct the façade's common model wire.
pub fn parse(input: &str, format: &str, _options: &Value) -> Result<Value, Value> {
    if format == "html" {
        return Err(
            json!({"code":"INCOMPLETE_HTML_GRAMMAR","message":"Source HTML input requires its math-markup grammar and captured case-insensitive closing tags; XML MathML extraction follows different source semantics"}),
        );
    }

    let xml = Xml {
        rows: source_document_rows(input)?,
    };
    if xml.rows.is_empty() {
        return Err(error("XML document has no element"));
    }
    let mut result = match format {
        "mathml" => {
            if xml.name(0) != "math" || (!xml.ns(0).is_empty() && xml.ns(0) != MATHML) {
                return Err(error("document root is not a MathML math element"));
            }
            xml.math(0)?
        }
        "omml" => {
            if xml.ns(0) != OMML || !matches!(xml.name(0), "oMath" | "oMathPara") {
                return Err(error(
                    "document root is not an OMML oMath or oMathPara element",
                ));
            }
            xml.omml(0)?
        }
        _ => return Err(error(format!("XML input format {format} is incomplete"))),
    };
    result["fields"]["input_string"] = json!(input);
    Ok(result)
}

#[cfg(test)]
#[path = "xml_parse/source_tests.rs"]
mod source_tests;
