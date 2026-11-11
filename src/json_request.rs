//! Bounded transport decoding, independent of notation/parser depth budgets.
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) const MAX_JSON_DEPTH: usize = 1024;
pub(crate) const MAX_JSON_NODES: usize = 1_000_000;

pub(crate) fn decode(bytes: &[u8]) -> Result<Value, Value> {
    // Only count structural nesting here. Serde remains the JSON syntax parser.
    // Brackets inside strings and escaped quotes must not consume this budget.
    let (mut quoted, mut escaped, mut depth, mut nodes, mut primitive) = (false, false, 0usize, 0usize, false);
    for &byte in bytes {
        if quoted {
            if escaped { escaped = false; }
            else if byte == b'\\' { escaped = true; }
            else if byte == b'"' { quoted = false; }
        } else {
            let starts_node = match byte {
                b'"' | b'{' | b'[' => { primitive = false; true }
                b'}' | b']' | b',' | b':' | b' ' | b'\n' | b'\r' | b'\t' => { primitive = false; false }
                _ => { let start = !primitive; primitive = true; start }
            };
            if starts_node {
                nodes += 1;
                if nodes > MAX_JSON_NODES {
                    return Err(json!({"abi_version":1,"ok":false,"error":{"code":"resource_limit",
                        "message":"request JSON node limit exceeded","resource":{"kind":"json_nodes","nodes":nodes,"max_nodes":MAX_JSON_NODES}}}));
                }
            }
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_JSON_DEPTH {
                        return Err(json!({"abi_version":1,"ok":false,"error":{
                            "code":"resource_limit","message":"request JSON nesting limit exceeded",
                            "resource":{"kind":"json_nesting","depth":depth,"max_depth":MAX_JSON_DEPTH}
                        }}));
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    // RawValue's pinned Serde implementation validates with ignore_value's
    // iterative scratch stack. Value::deserialize recurses and is not used here.
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let raw = <&serde_json::value::RawValue>::deserialize(&mut decoder)
        .and_then(|value| decoder.end().map(|_| value))
        .map_err(invalid_json)?;
    let offset = raw.get().as_ptr() as usize - bytes.as_ptr() as usize;
    assemble_validated(raw.get().as_bytes(), bytes, offset)
}

fn invalid_json(error: serde_json::Error) -> Value {
    json!({"abi_version":1,"ok":false,"error":{"code":"invalid_json","message":error.to_string()}})
}

fn invalid_json_at(error: serde_json::Error, original: &[u8], start: usize) -> Value {
    if error.line() == 0 { return invalid_json(error); }
    let prefix = &original[..start];
    let line = prefix.iter().filter(|&&byte| byte == b'\n').count() + error.line();
    let column = if error.line() == 1 {
        start - prefix.iter().rposition(|&byte| byte == b'\n').map_or(0, |index|index+1) + error.column()
    } else { error.column() };
    let text = error.to_string();
    let suffix = format!(" at line {} column {}",error.line(),error.column());
    let message = text.strip_suffix(&suffix).unwrap_or(&text);
    json!({"abi_version":1,"ok":false,"error":{"code":"invalid_json","message":format!("{message} at line {line} column {column}")}})
}

enum Frame {
    Array(Vec<Value>),
    Object(serde_json::Map<String, Value>, Option<String>, usize),
}
fn attach(value: Value, frames: &mut [Frame], root: &mut Option<Value>) {
    match frames.last_mut() {
        Some(Frame::Array(values)) => values.push(value),
        Some(Frame::Object(values, key, _)) => { values.insert(key.take().expect("validated object key"), value); }
        None => *root = Some(value),
    }
}

// This assembles already-validated JSON, not a second syntax parser. Strings
// and scalar numbers are still decoded by Serde; containers live on the heap.
fn assemble_validated(bytes: &[u8], original: &[u8], offset: usize) -> Result<Value, Value> {
    let mut frames = Vec::new();
    let mut root = None;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => { frames.push(Frame::Object(serde_json::Map::new(), None, index)); index += 1; }
            b'[' => { frames.push(Frame::Array(Vec::new())); index += 1; }
            b'}' | b']' => {
                let value = match frames.pop().expect("validated container") {
                    Frame::Array(values) => Value::Array(values),
                    Frame::Object(values, _, start) => {
                        // Keep the pre-existing arbitrary_precision private-number
                        // mapping; the newly enabled RawValue marker stays literal.
                        if values.keys().next().is_some_and(|key|key == "$serde_json::private::Number") {
                            serde_json::from_slice(&bytes[start..index+1]).map_err(|error|invalid_json_at(error,original,offset+start))?
                        } else { Value::Object(values) }
                    },
                };
                attach(value, &mut frames, &mut root);
                index += 1;
            }
            b'"' => {
                let start = index;
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if escaped { escaped = false; }
                    else if byte == b'\\' { escaped = true; }
                    else if byte == b'"' { break; }
                }
                let value: String = serde_json::from_slice(&bytes[start..index]).map_err(|error|invalid_json_at(error,original,offset+start))?;
                if let Some(Frame::Object(_, key @ None, _)) = frames.last_mut() {
                    *key = Some(value);
                } else {
                    attach(Value::String(value), &mut frames, &mut root);
                }
            }
            b' ' | b'\n' | b'\r' | b'\t' | b':' | b',' => index += 1,
            _ => {
                let start = index;
                while index < bytes.len() && !matches!(bytes[index], b' ' | b'\n' | b'\r' | b'\t' | b',' | b'}' | b']') {
                    index += 1;
                }
                let value = serde_json::from_slice(&bytes[start..index]).map_err(|error|invalid_json_at(error,original,offset+start))?;
                attach(value, &mut frames, &mut root);
            }
        }
    }
    Ok(root.expect("validated root value"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn syntax_remains_serde_and_quoted_brackets_do_not_count() {
        let input = json!({"literal":"[{}]\"\\".repeat(700)}).to_string();
        assert_eq!(decode(input.as_bytes()).unwrap()["literal"], json!("[{}]\"\\".repeat(700)));
        for input in [b"{]".as_slice(), b"{} true".as_slice(), b"[1,]".as_slice()] {
            assert_eq!(decode(input).unwrap_err()["error"]["code"], "invalid_json");
        }
    }
    #[test]
    fn nesting_resource_is_structured_before_deserialization() {
        let input = format!("{}0{}", "[".repeat(MAX_JSON_DEPTH + 1), "]".repeat(MAX_JSON_DEPTH + 1));
        let error=decode(input.as_bytes()).unwrap_err();
        assert_eq!(error["error"]["code"],"resource_limit");
        assert_eq!(error["error"]["resource"]["kind"],"json_nesting");
        assert_eq!(error["error"]["resource"]["max_depth"],MAX_JSON_DEPTH);
        assert_eq!(decode(b"{\"op\":\"model\"}").unwrap()["op"],"model");
    }
    #[test]
    fn deep_validated_assembly_preserves_values_and_object_order() {
        let mut value:Value=serde_json::from_str(r#"{"z":"a[\\\"b","n":9007199254740993123456789,"d":-0.0,"t":true,"none":null,"z":"last"}"#).unwrap();
        for _ in 0..160 { let mut object=serde_json::Map::new();object.insert("next".into(),Value::Array(vec![value]));value=Value::Object(object); }
        let leaf=r#"{"z":"a[\\\"b","n":9007199254740993123456789,"d":-0.0,"t":true,"none":null,"z":"last"}"#;
        let bytes=format!("{}{}{}","{\"next\":[".repeat(160),leaf,"]}".repeat(160));
        let actual=decode(bytes.as_bytes()).unwrap();assert_eq!(actual,value);
        assert_eq!(serde_json::to_vec(&actual).unwrap(),serde_json::to_vec(&value).unwrap());
    }

}
