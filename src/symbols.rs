use serde_json::Value;
use std::sync::OnceLock;
pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("symbols.json")).expect("source generated symbol data")
    })
}
pub fn symbol(text: &str) -> Value {
    if let Some(id) = tables()["input"][text].as_str() {
        crate::model::node(
            &format!("Math::Symbols::{id}"),
            serde_json::json!({"value":null}),
        )
    } else {
        crate::model::node("Math::Symbols::Symbol", serde_json::json!({"value":text}))
    }
}

/// Format-specific source lookup; avoid merging namespaces into AsciiMath.
pub fn symbol_for(text: &str, format: &str) -> Value {
    let key = match format {
        "latex" => "latex_input",
        "unicode" | "unicodemath" => "unicode_input",
        _ => "input",
    };
    let normalized =
        text.trim_matches(|c| matches!(c, '\0' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' '));
    if let Some(id) = tables()[key][normalized].as_str() {
        crate::model::node(
            &format!("Math::Symbols::{id}"),
            serde_json::json!({"value":null}),
        )
    } else {
        crate::model::node("Math::Symbols::Symbol", serde_json::json!({"value":text}))
    }
}
