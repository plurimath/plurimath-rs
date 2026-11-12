use plurimath_facade::request;
use serde_json::{json, Value};
#[test]
fn actual_source_font_family_writers_and_typed_refusals() {
    let source: Value = serde_json::from_str(include_str!(
        "../fixtures/font-styles/source-reference.json"
    ))
    .unwrap();
    let mut failures = vec![];
    let mut outputs = 0;
    let mut refusals = 0;
    for row in source["rows"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        for (format, expected) in row["writers"].as_object().unwrap() {
            let actual =
                request(&json!({"op":"write","model":row["model"],"format":format,"options":{}}));
            if let Some(output) = expected.get("output") {
                outputs += 1;
                if actual["ok"] != true || actual["output"] != *output {
                    failures
                        .push(json!({"id":id,"format":format,"actual":actual,"expected":expected}));
                }
            } else {
                refusals += 1;
                if actual["ok"] != false
                    || actual["error"]["source_class"] != expected["error"]["class"]
                {
                    failures
                        .push(json!({"id":id,"format":format,"actual":actual,"expected":expected}));
                }
            }
        }
    }
    assert_eq!(outputs, 1127);
    assert_eq!(refusals, 253);
    if !failures.is_empty() {
        if let Some(path) = std::env::var_os("FONT_DIFFERENTIAL_OUTPUT") {
            std::fs::write(path, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
        }
    }
    assert!(
        failures.is_empty(),
        "source font mismatches: {} (full differential saved)",
        failures.len()
    );
}
