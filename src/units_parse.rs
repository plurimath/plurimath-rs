//! Source UnitsML input through the accepted AsciiUnits stage and UnitsDB records.
use crate::model::{formula as new_formula, node};
use moxml::document::{ContentRow, ContentSpec, Document, ElementSpec};
use serde_json::{json, Value};
use unitsdb_rs::Lookup;

fn incomplete(message: impl ToString) -> Value {
    json!({"code":"INCOMPLETE_UNITSML_TRANSLATION","message":message.to_string()})
}
fn syntax(message: impl ToString) -> Value {
    json!({"code":"PARSE_ERROR","message":message.to_string(),"byte_offset":0,"offset_policy":"source-unitsml-facade-zero"})
}
fn normalized(input: &str) -> Result<String, Value> {
    let text = if let Some(body) = input.strip_prefix("unitsml(") {
        body.strip_suffix(')').unwrap_or(body)
    } else {
        input
    };
    if ["quantity:", "name:", "symbol:", "multiplier:"]
        .iter()
        .any(|key| text.contains(key))
    {
        return Err(incomplete(
            "UnitsML declaration extras require the source metadata renderer",
        ));
    }
    Ok(text.replace('−', "-"))
}
fn stage(input: &str) -> Result<Value, Value> {
    let text = normalized(input)?;
    let parser = asciiunits::Parser::new().map_err(|error| incomplete(error.to_string()))?;
    parser.parse(&text).map_err(|error| {
        let details = serde_json::to_value(&error).unwrap_or(Value::Null);
        if matches!(
            details["category"].as_str(),
            Some("recursion_limit" | "call_depth_limit")
        ) {
            json!({"code":"RESOURCE_LIMIT","message":error.to_string(),"stage_error":details})
        } else {
            syntax(error.to_string())
        }
    })
}
fn unit_symbol(name: &str) -> Result<&'static Value, Value> {
    let record = unitsdb_rs::find(Lookup::UnitSymbol, name)
        .ok_or_else(|| syntax(format!("unknown source unit {name}")))?;
    record["symbols"]
        .as_array()
        .and_then(|symbols| symbols.iter().find(|symbol| symbol["id"] == name))
        .ok_or_else(|| syntax(format!("unknown source unit symbol id {name}")))
}
fn validate(value: &Value) -> Result<(), Value> {
    if let Some(name) = value["unitName"].as_str() {
        unit_symbol(name)?;
    }
    if let Some(fields) = value.as_object() {
        for child in fields.values() {
            validate(child)?;
        }
    } else if let Some(values) = value.as_array() {
        for child in values {
            validate(child)?;
        }
    }
    Ok(())
}
/// The source façade stores text on Unitsml and wraps its bare node in Formula.
pub fn parse(input: &str, _options: &Value) -> Result<Value, Value> {
    let value = stage(input)?;
    validate(&value)?;
    let mut result = new_formula(vec![node("Math::Function::Unitsml", json!({"text":input}))]);
    result["fields"]["input_string"] = json!(input);
    Ok(result)
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn number_xml(value: &str) -> String {
    if let Some(absolute) = value.strip_prefix('-') {
        format!("<mrow><mo>−</mo><mn>{}</mn></mrow>", escape(absolute))
    } else {
        format!("<mn>{}</mn>", escape(value))
    }
}
fn exponent(value: &Value, inverse: bool, square_root: bool) -> Result<Option<String>, Value> {
    if square_root {
        return Ok(Some("0.5".into()));
    }
    let power = if value.is_null() {
        None
    } else {
        Some(
            value["value"]
                .as_str()
                .ok_or_else(|| {
                    incomplete("structured UnitsML exponent requires its source numeric renderer")
                })?
                .to_owned(),
        )
    };
    Ok(if inverse {
        Some(match power {
            None => "-1".into(),
            Some(s) => s
                .strip_prefix('-')
                .map_or_else(|| format!("-{s}"), str::to_owned),
        })
    } else {
        power
    })
}
fn with_prefix(fragment: &str, prefix: &Value) -> Result<String, Value> {
    if prefix.is_null() {
        return Ok(fragment.into());
    }
    let name = prefix["prefixName"]
        .as_str()
        .ok_or_else(|| incomplete("UnitsML prefix has no source name"))?;
    let record = unitsdb_rs::find(Lookup::PrefixSymbol, name)
        .or_else(|| unitsdb_rs::find(Lookup::PrefixId, name))
        .ok_or_else(|| syntax(format!("unknown prefix {name}")))?;
    let html = record["symbols"]
        .as_array()
        .and_then(|symbols| symbols.last())
        .and_then(|symbol| symbol["html"].as_str())
        .ok_or_else(|| incomplete("UnitsDB prefix has no HTML symbol"))?;
    let prefix = crate::xml_parse::source_entity_decode(html)?;
    let rows = crate::xml_parse::source_document_rows(fragment)?;
    let root = rows
        .first()
        .ok_or_else(|| incomplete("unit MathML has no root"))?;
    if root
        .content
        .iter()
        .any(|c| matches!(c, ContentRow::Element(_)))
    {
        return Err(incomplete(
            "prefixing a composite UnitsDB MathML root requires the source typed-content mapper",
        ));
    }
    let text = root
        .content
        .iter()
        .filter_map(|c| match c {
            ContentRow::Text(t) | ContentRow::CData(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<String>();
    let root = ElementSpec {
        name: root.name.clone(),
        namespaces: root.namespaces.clone(),
        attributes: root.attributes.clone(),
        content: vec![ContentSpec::Text(prefix + &text)],
    };
    Document::build(root)
        .and_then(|doc| doc.to_xml())
        .map_err(|error| incomplete(error.to_string()))
}
fn render(value: &Value, inverse: bool, square_root: bool) -> Result<String, Value> {
    if let Some(name) = value["unitName"].as_str() {
        let symbol = unit_symbol(name)?;
        let fragment = symbol["mathml"]
            .as_str()
            .ok_or_else(|| incomplete("UnitsDB unit has no MathML symbol"))?;
        let mut output = with_prefix(fragment, &value["prefix"])?;
        if let Some(power) = exponent(&value["powerNumerator"], inverse, square_root)? {
            output = format!("<msup>{output}{}</msup>", number_xml(&power));
        }
        return Ok(output);
    }
    if value.get("dimensionName").is_some() {
        return Err(incomplete(
            "UnitsML dimension rendering requires the source dimension option policy",
        ));
    }
    if value.get("open").is_some() {
        return render(&value["value"], inverse, square_root);
    }
    if let Some(values) = value["value"].as_array() {
        let mut output = String::new();
        let mut inverse = inverse;
        for item in values {
            if let Some(separator) = item["text"].as_str() {
                if matches!(separator, "/" | "//") {
                    inverse = !inverse;
                }
                if matches!(separator, "/" | "//" | "*")
                    || separator.chars().all(char::is_whitespace)
                {
                    output.push_str("<mo>⋅</mo>");
                } else {
                    return Err(incomplete(format!(
                        "UnitsML extender {separator} requires its source renderer"
                    )));
                }
            } else {
                output.push_str(&render(item, inverse, square_root)?);
            }
        }
        return Ok(output);
    }
    if value["value"].is_object() {
        if value["value"].get("unitName").is_none() {
            return Err(incomplete(
                "compound square-root exponent propagation requires the source recursive renderer",
            ));
        }
        return render(&value["value"], inverse, true);
    }
    if let Some(number) = value["value"].as_str() {
        return Ok(number_xml(number));
    }
    Err(incomplete(
        "accepted AsciiUnits node has no source UnitsML renderer yet",
    ))
}
/// Derive the same common formula the source Unitsml node renders on demand.
pub fn formula(input: &str, units_options: &Value) -> Result<Value, Value> {
    if units_options
        .as_object()
        .is_some_and(|options| !options.is_empty())
    {
        return Err(incomplete(
            "UnitsML custom rendering options require their source option policy",
        ));
    }
    let value = stage(input)?;
    validate(&value)?;
    let body = render(&value, false, false)?;
    crate::xml_parse::parse(
        &format!("<math display=\"block\">{body}</math>"),
        "mathml",
        &json!({}),
    )
}
