use serde_json::{json, Value};
#[test]
fn source_parse_options_precede_grammar_and_preserve_refusal_classes() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/parse-options/source-cases.json")).unwrap();
    for row in corpus["rows"].as_array().unwrap() {
        let result=plurimath_facade::request_json(&serde_json::to_vec(&json!({"op":"parse","format":row["input"]["format"],"input":row["input"]["text"],"options":row["parseOptions"]})).unwrap());
        assert_eq!(result["ok"], false, "{}: {}", row["id"], result);
        assert_eq!(
            result["error"]["source_class"], row["raises"],
            "{}: {}",
            row["id"], result
        );
    }
    assert_eq!(corpus["rows"].as_array().unwrap().len(), 7);
}
