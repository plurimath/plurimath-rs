//! Exact structural script models from pinned Ruby 0a1df68.
use serde_json::{json, Value};

/// The facade records the source text on the root Formula; fixtures omit it.
fn without_input_string(mut model: Value) -> Value {
    if let Some(fields) = model.get_mut("fields").and_then(Value::as_object_mut) {
        fields.remove("input_string");
    }
    model
}

#[test]
fn unicode_structural_scripts_match_source_models() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/unicode-structural-scripts/source-models.json"
    ))
    .unwrap();
    let mut differences = Vec::new();
    for row in fixture["rows"].as_array().unwrap() {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"unicode","input":row["input"],"options":{}}),
        );
        if actual["ok"] != true || without_input_string(actual["model"].clone()) != row["expected"]
        {
            differences
                .push(json!({"input":row["input"],"expected":row["expected"],"actual":actual}));
        }
    }
    assert!(
        differences.is_empty(),
        "source model differences: {differences:#?}"
    );
}
