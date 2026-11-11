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
