//! Measured intent, argument and uncertainty-marker models from Ruby 0a1df68.
use serde_json::{json, Value};

#[test]
fn unicode_intents_arguments_and_markers_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/unicode-intent/source-models.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert!(!rows.is_empty());
    let mut differences = Vec::new();
    for row in rows {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"unicode","input":row["input"],"options":{}}),
        );
        let matches = if row.get("expected").is_some() {
            actual["ok"] == true && actual["model"] == row["expected"]
        } else {
            actual["ok"] == false
                && actual["error"]["code"] == "PARSE_ERROR"
                && (actual["error"]["cause_class"] == row["expected_error"]
                    || actual["error"]["source_class"] == row["expected_error"])
        };
        if !matches {
            differences.push(json!({"input":row["input"],"expected":row,"actual":actual}));
        }
    }
    assert!(
        differences.is_empty(),
        "{} source differences; first observations: {:#?}",
        differences.len(),
        differences.iter().take(8).collect::<Vec<_>>()
    );
}
