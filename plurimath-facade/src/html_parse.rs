//! Existing Ruby HTML grammar graph, portable captured-close operation, source normalization.
mod transform;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

fn error(code: &str, message: impl Into<String>) -> Value {
    json!({"code":code,"message":message.into(),"stage":"html"})
}
fn normalize(input: &str) -> Result<String, Value> {
    static ENTITY: OnceLock<Regex> = OnceLock::new();
    let pattern =
        ENTITY.get_or_init(|| Regex::new(r"(?i)&(?:#x[0-9a-f]+|#[0-9]+|[a-z][a-z0-9]+);").unwrap());
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;
    for found in pattern.find_iter(input) {
        result.push_str(&input[cursor..found.start()]);
        let token = found.as_str();
        let body = &token[1..token.len() - 1];
        let numeric = if body.starts_with("#x") || body.starts_with("#X") {
            let digits = &body[2..];
            if digits.len() <= 6 {
                u32::from_str_radix(digits, 16).ok()
            } else {
                None
            }
        } else if let Some(digits) = body.strip_prefix('#') {
            if digits.len() <= 7 {
                digits.parse::<u32>().ok()
            } else {
                None
            }
        } else {
            crate::symbols::tables()["html_entities"][body]
                .as_u64()
                .map(|v| v as u32)
        };
        let decoded = if let Some(codepoint) = numeric {
            let Some(character) = char::from_u32(codepoint) else {
                let message = if (0xd800..=0xdfff).contains(&codepoint) {
                    format!("invalid codepoint 0x{codepoint:X} in UTF-8")
                } else {
                    format!("{codepoint} out of char range")
                };
                return Err(
                    json!({"code":"normalization_error","source_class":"RangeError","message":message,"stage":"html"}),
                );
            };
            character.to_string()
        } else {
            token.to_owned()
        };
        for character in decoded.chars() {
            let point = character as u32;
            if [0x3c, 0x3e, 0x26, 0x22, 0x27].contains(&point) || !(0x20..=0x7e).contains(&point) {
                result.push_str(&format!("&#x{point:x};"));
            } else {
                result.push(character);
            }
        }
        cursor = found.end();
    }
    result.push_str(&input[cursor..]);
    Ok(result)
}
#[derive(Clone, Copy)]
enum Origin {
    Reference,
    Map,
    Other,
}
struct Lowered {
    value: Value,
    origin: Origin,
}
fn captures(root: &Value, input: &str) -> Result<Value, Value> {
    enum Work<'a> {
        Visit(&'a Value),
        Array(usize),
        Object(Vec<String>),
        Tagged(&'a str, usize),
    }
    let mut work = vec![Work::Visit(root)];
    let mut out = Vec::<Lowered>::new();
    while let Some(task) = work.pop() {
        match task {
            Work::Visit(Value::Array(values)) => {
                work.push(Work::Array(values.len()));
                for value in values.iter().rev() {
                    work.push(Work::Visit(value));
                }
            }
            Work::Visit(Value::Object(fields)) => {
                let reference = fields.get("offset").is_some_and(Value::is_u64)
                    && fields.get("length").is_some_and(Value::is_u64)
                    && fields.keys().all(|k| {
                        ["offset", "length", "value", "line", "column"].contains(&k.as_str())
                    });
                if reference {
                    let text = if let Some(value) = fields.get("value").and_then(Value::as_str) {
                        value.to_owned()
                    } else {
                        let offset = fields["offset"].as_u64().unwrap() as usize;
                        let length = fields["length"].as_u64().unwrap() as usize;
                        input
                            .get(offset..)
                            .ok_or_else(|| {
                                error("invalid_native_shape", "HTML slice byte boundary")
                            })?
                            .chars()
                            .take(length)
                            .collect()
                    };
                    out.push(Lowered {
                        value: Value::String(text),
                        origin: Origin::Reference,
                    });
                } else if fields.len() == 2
                    && fields.get("tag").is_some_and(Value::is_string)
                    && fields.get("items").is_some_and(Value::is_array)
                {
                    let values = fields["items"].as_array().unwrap();
                    work.push(Work::Tagged(
                        fields["tag"].as_str().unwrap().trim_start_matches(':'),
                        values.len(),
                    ));
                    for value in values.iter().rev() {
                        work.push(Work::Visit(value));
                    }
                } else {
                    work.push(Work::Object(fields.keys().cloned().collect()));
                    for value in fields.values().rev() {
                        work.push(Work::Visit(value));
                    }
                }
            }
            Work::Visit(value) => out.push(Lowered {
                value: value.clone(),
                origin: Origin::Other,
            }),
            Work::Array(count) => {
                let values = out
                    .split_off(out.len() - count)
                    .into_iter()
                    .map(|v| v.value)
                    .collect();
                out.push(Lowered {
                    value: Value::Array(values),
                    origin: Origin::Other,
                });
            }
            Work::Object(keys) => {
                let values = out
                    .split_off(out.len() - keys.len())
                    .into_iter()
                    .map(|v| v.value);
                out.push(Lowered {
                    value: Value::Object(keys.into_iter().zip(values).collect()),
                    origin: Origin::Map,
                });
            }
            Work::Tagged(tag, count) => {
                let values = out.split_off(out.len() - count);
                let value = match tag {
                    "repetition" => Lowered {
                        value: Value::Array(values.into_iter().map(|v| v.value).collect()),
                        origin: Origin::Other,
                    },
                    "maybe" => values.into_iter().next().unwrap_or(Lowered {
                        value: Value::Null,
                        origin: Origin::Other,
                    }),
                    "sequence" => {
                        let mut values: Vec<_> =
                            values.into_iter().filter(|v| !v.value.is_null()).collect();
                        if values.is_empty() {
                            Lowered {
                                value: Value::Null,
                                origin: Origin::Other,
                            }
                        } else if values.len() == 1 {
                            values.remove(0)
                        } else if values.iter().any(|v| matches!(v.origin, Origin::Map))
                            && values
                                .iter()
                                .all(|v| matches!(v.origin, Origin::Map | Origin::Reference))
                        {
                            let mut fields = Map::new();
                            for v in values {
                                if matches!(v.origin, Origin::Map) {
                                    if let Value::Object(owned)=v.value {fields.extend(owned);}
                                }
                            }
                            Lowered {
                                value: Value::Object(fields),
                                origin: Origin::Map,
                            }
                        } else {
                            Lowered {
                                value: Value::Array(values.into_iter().map(|v| v.value).collect()),
                                origin: Origin::Other,
                            }
                        }
                    }
                    _ => {
                        return Err(error(
                            "invalid_native_shape",
                            format!("unknown tagged HTML capture {tag}"),
                        ))
                    }
                };
                out.push(value);
            }
        }
    }
    let value = out
        .pop()
        .ok_or_else(|| error("invalid_native_shape", "empty HTML shape"))?
        .value;
    // Exactly Html::Parser#normalized_nodes: only Hash/Array at depth>=100 fail.
    let mut nodes = vec![(&value, 0usize)];
    while let Some((node, depth)) = nodes.pop() {
        if depth >= 100 && (node.is_array() || node.is_object()) {
            return Err(
                json!({"code":"resource_limit","source_class":"JSON::NestingError","message":format!("nesting of {} is too deep",depth+1),"resource":{"kind":"html_capture_nesting","depth":depth+1,"max_depth":100},"stage":"html"}),
            );
        }
        match node {
            Value::Array(values) => {
                for v in values {
                    nodes.push((v, depth + 1));
                }
            }
            Value::Object(fields) => {
                for v in fields.values() {
                    nodes.push((v, depth + 1));
                }
            }
            _ => {}
        }
    }
    Ok(value)
}
fn parse_error(e: parsanol::PargError) -> Value {
    let resource = match &e {
        parsanol::PargError::RecursionLimitExceeded { depth, max_depth } => {
            Some(json!({"kind":"recursion_limit","depth":depth,"max_depth":max_depth}))
        }
        parsanol::PargError::CallDepthLimitExceeded { depth, max_depth } => {
            Some(json!({"kind":"call_depth_limit","depth":depth,"max_depth":max_depth}))
        }
        _ => None,
    };
    let offset = match &e {
        parsanol::PargError::ParseWire { offset, .. } => Some(*offset),
        _ => None,
    };
    json!({"code":if resource.is_some(){"resource_limit"}else if offset.is_some(){"PARSE_ERROR"}else{"parser_error"},"message":e.to_string(),"stage":"html","byte_offset":offset,"offset_policy":"normalized_html_utf8_bytes","resource":resource})
}
pub fn parse(input: &str, options: &Value) -> Result<Value, Value> {
    if !options.as_object().is_some_and(|v| v.is_empty()) {
        return Err(error(
            "INCOMPLETE_HTML_OPTIONS",
            "HTML source graph currently declares default options and decimal period",
        ));
    }
    static ARTIFACT: OnceLock<Result<parsanol::PargArtifact, String>> = OnceLock::new();
    let artifact = match ARTIFACT.get_or_init(|| {
        parsanol::PargArtifact::from_json(include_str!(
            "../fixtures/html-parse/source-artifact.json"
        ))
        .map_err(|e| e.to_string())
    }) {
        Ok(a) => a,
        Err(e) => return Err(error("parser_initialization_error", e.clone())),
    };
    let normalized = normalize(input)?;
    let shape = artifact
        .parse_shape_with_compatibility(
            "document",
            &normalized,
            parsanol::portable::parser::ParserCompatibility::Strict,
        )
        .map_err(parse_error)?;
    let tree = captures(&shape, &normalized)?;
    let mut model = transform::transform(&tree, input)?;
    model["fields"]["input_string"] = json!(input);
    Ok(model)
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn source_normalized_entities() {
        for (input, expected) in [
            ("&lt;", "&#x3c;"),
            ("&#x2b;", "+"),
            ("&unknown;", "&#x26;unknown;"),
            ("&COPY;", "&#x26;COPY;"),
            ("&#X2B;", "+"),
            ("&nbsp;", "&#xa0;"),
            ("&#00000065;", "&#x26;#00000065;"),
        ] {
            assert_eq!(normalize(input).unwrap(), expected);
        }
        assert_eq!(
            normalize("&#xD800;").unwrap_err()["source_class"],
            "RangeError"
        );
    }
}
