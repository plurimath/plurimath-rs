use serde_json::{json, Value};

#[test]
fn source_binary_defaults_scripts_and_unseen_children() {
    let reference: Value =
        serde_json::from_str(include_str!("../fixtures/ascii-extra858/reference.json")).unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let result = plurimath_facade::request(&json!({
            "op": "parse", "format": "asciimath", "input": input, "options": {}
        }));
        assert_eq!(
            result["model"], case["model"],
            "source input: {input}; response: {result}"
        );
    }
}
