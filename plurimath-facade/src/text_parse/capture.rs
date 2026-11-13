//! LaTeX `nil` captures: `{}` is the source absent wrapper, which builds nothing,
//! and a unary over it is `Utility.get_class(u).new(nil)` (Ruby 0a1 `Latex::Mapper`).
use super::{tx, unary};
use serde_json::{Map, Value};

/// Returns `Ok(None)` when the capture is not one of these shapes.
pub(super) fn parse(
    format: &str,
    m: &Map<String, Value>,
    depth: usize,
) -> Result<Option<Vec<Value>>, Value> {
    if m.len() == 1 && m.get("nil") == Some(&Value::Bool(true)) {
        return Ok(Some(Vec::new()));
    }
    if format == "latex" && m.len() == 2 {
        if let (Some(name), Some(operand)) =
            (m.get("unary").and_then(Value::as_str), m.get("first_value"))
        {
            if tx(format, operand, depth + 1)?.is_empty() {
                return Ok(Some(vec![unary(name, Value::Null)?]));
            }
        }
    }
    Ok(None)
}
