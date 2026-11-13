use plurimath_facade::request;
use serde_json::{json, Value};
#[test]
fn source_stacked_and_glyph_writers() {
    let source: Value = serde_json::from_str(include_str!(
        "../fixtures/stacked-writers/source-reference.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    let (mut exact, mut exceptions) = (0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            let actual =
                request(&json!({"op":"write","model":row["model"],"format":format,"options":{}}));
            let matches = if expected.get("output").is_some() {
                let ok = actual["ok"] == true && actual["output"] == expected["output"];
                if ok {
                    exact += 1;
                }
                ok
            } else {
                let ok = actual["ok"] == false
                    && actual["error"]["source_class"] == expected["error"]["class"];
                if ok {
                    exceptions += 1;
                }
                ok
            };
            if !matches {
                failures.push(
                    json!({"id":row["id"],"format":format,"actual":actual,"expected":expected}),
                );
            }
        }
    }
    println!(
        "{}",
        json!({"exact":exact,"source_refusals":exceptions,"failures":failures.len()})
    );
    if let Some(p) = std::env::var_os("STACKED_DIFFERENTIAL_OUTPUT") {
        std::fs::write(p, serde_json::to_vec(&failures).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} mismatches; bounded IDs {:?}",
        failures.len(),
        failures
            .iter()
            .take(6)
            .map(|v| (&v["id"], &v["format"]))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        exact + exceptions,
        source["rows"].as_array().unwrap().len() * 6
    );
}

#[test]
fn current_ruby_xml_writer_regressions() {
    let source: Value = serde_json::from_str(include_str!(
        "../fixtures/stacked-writers/xml-source-writer-regressions.json"
    ))
    .unwrap();
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            let actual = request(
                &json!({"op":"write","model":row["parsed_model"],"format":format,"options":{}}),
            );
            if source["regression_targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["id"] == row["id"] && t["format"] == format.as_str())
            {
                assert_eq!(
                    actual["ok"], true,
                    "required fix {} {format}: {actual}",
                    row["id"]
                );
            }
            if actual["ok"] == true {
                assert_eq!(
                    &actual["output"],
                    expected.get("output").unwrap_or(expected),
                    "{} {format}",
                    row["id"]
                );
            }
        }
    }
}
