//! Source common-node wire; fields deliberately retain null, arrays and plain hashes.
use serde_json::{Map, Value};
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub class: String,
    pub fields: Map<String, Value>,
}
impl Model {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or("model must be an object")?;
        if o.keys().any(|k| k != "class" && k != "fields") {
            return Err("unknown model envelope member".into());
        }
        let class = o
            .get("class")
            .and_then(Value::as_str)
            .ok_or("model.class must be a string")?
            .to_owned();
        if !class.starts_with("Math::") {
            return Err("model.class must use the source Math:: class name".into());
        }
        let fields = o
            .get("fields")
            .and_then(Value::as_object)
            .ok_or("model.fields must be an object")?
            .clone();
        Ok(Self { class, fields })
    }
    pub fn value(&self) -> Value {
        serde_json::json!({"class":self.class,"fields":self.fields})
    }
    pub fn field(&self, k: &str) -> &Value {
        self.fields.get(k).unwrap_or(&Value::Null)
    }
    pub fn kind(&self) -> &str {
        self.class.rsplit("::").next().unwrap_or("")
    }
}
pub fn node(class: &str, fields: Value) -> Value {
    serde_json::json!({"class":class,"fields":fields})
}
pub fn formula(value: Vec<Value>) -> Value {
    node(
        "Math::Formula",
        serde_json::json!({"displaystyle":true,"left_right_wrapper":true,"value":value}),
    )
}
pub fn number(value: &str) -> Value {
    node(
        "Math::Number",
        serde_json::json!({"base":null,"mini_sub_sized":false,"mini_sup_sized":false,"value":value}),
    )
}
