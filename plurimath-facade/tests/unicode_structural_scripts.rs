//! Exact structural script models from pinned Ruby 0a1df68.
use serde_json::{json, Value};

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
        if actual["ok"] != true || actual["model"] != row["expected"] {
            differences
                .push(json!({"input":row["input"],"expected":row["expected"],"actual":actual}));
        }
    }
    assert!(
        differences.is_empty(),
        "source model differences: {differences:#?}"
    );
}
