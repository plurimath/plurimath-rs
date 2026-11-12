use serde_json::{json, Value};
#[test]
fn source_backed_remaining_asciimath_models() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/asciimath-source-remaining.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let result = plurimath_facade::request(
            &json!({"op":"parse","format":"asciimath","input":case["input"],"options":{}}),
        );
        assert_eq!(result["model"], case["model"], "{}: {}", case["id"], result);
    }
}
