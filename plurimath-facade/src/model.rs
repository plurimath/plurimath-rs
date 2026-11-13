//! Source common-node wire; fields deliberately retain null, arrays and plain hashes.
use serde_json::{Map, Value};
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub class: String,
    pub fields: Map<String, Value>,
    pub source_values: Option<crate::source_values::Metadata>,
}
impl Model {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or("model must be an object")?;
        if o.keys()
            .any(|k| k != "class" && k != "fields" && k != "source_values")
        {
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
        let source_values = crate::source_values::metadata(o.get("source_values"))?;
        if let Some(metadata) = &source_values {
            crate::source_values::verify_projection(&class, &fields, metadata)?;
        }
        Ok(Self {
            class,
            fields,
            source_values,
        })
    }
    pub fn value(&self) -> Value {
        {
            let mut value = serde_json::json!({"class":self.class,"fields":self.fields});
            if let Some(metadata) = &self.source_values {
                value["source_values"] = serde_json::json!({"schema":crate::source_values::SCHEMA,"fields":metadata.fields});
            }
            value
        }
    }
    pub fn field(&self, k: &str) -> &Value {
        self.fields.get(k).unwrap_or(&Value::Null)
    }
    pub fn profile(&self) -> crate::source_values::Profile {
        if self.source_values.is_some() {
            crate::source_values::Profile::RubyValuesV1
        } else {
            crate::source_values::Profile::Canonical
        }
    }
    pub fn source_field(&self, name: &str) -> &Value {
        if let Some(metadata) = &self.source_values {
            return metadata
                .fields
                .get(name)
                .unwrap_or_else(|| crate::source_values::nil_tag());
        }
        self.field(name)
    }
    pub fn kind(&self) -> &str {
        self.class.rsplit("::").next().unwrap_or("")
    }
}
pub fn node(class: &str, fields: Value) -> Value {
    serde_json::json!({"class":class,"fields":fields})
}
pub fn formula(value: Vec<Value>) -> Value {
    let wrapper = !value
        .first()
        .is_some_and(|v| v["class"] == "Math::Function::Left");
    node(
        "Math::Formula",
        serde_json::json!({"displaystyle":true,"left_right_wrapper":wrapper,"value":value}),
    )
}
pub fn number(value: &str) -> Value {
    node(
        "Math::Number",
        serde_json::json!({"base":null,"mini_sub_sized":false,"mini_sup_sized":false,"value":value}),
    )
}
