use serde_json::{json, Value};

#[test]
fn units_source_writer_bytes() {
    let fixtures = [
        include_str!("../fixtures/units-source/baseline.json"),
        include_str!("../fixtures/units-source/extended.json"),
        include_str!("../fixtures/units-source/independent.json"),
    ];
    let mut checked = 0;
    for fixture in fixtures {
        let source: Value = serde_json::from_str(fixture).unwrap();
        for row in source["rows"].as_array().unwrap() {
            let parsed = plurimath_facade::request(
                &json!({"op":"parse","format":"unitsml","input":row["input"],"options":{}}),
            );
            let Some(outputs) = row["outputs"].as_object() else {
                assert_eq!(
                    parsed["ok"], false,
                    "source rejection {}: {parsed}",
                    row["input"]
                );
                assert_eq!(
                    parsed["error"]["source_class"], row["error"],
                    "source rejection {}",
                    row["input"]
                );
                continue;
            };
            assert_eq!(parsed["ok"], true, "parse {}: {parsed}", row["input"]);
            for (format, expected) in outputs {
                let Some(expected) = expected.as_str() else {
                    continue;
                };
                let actual = plurimath_facade::request(
                    &json!({"op":"write","format":format,"model":parsed["model"],"options":{}}),
                );
                assert_eq!(actual["ok"], true, "{}/{}: {actual}", row["input"], format);
                assert_eq!(
                    actual["output"].as_str(),
                    Some(expected),
                    "{}/{}",
                    row["input"],
                    format
                );
                checked += 1;
            }
        }
    }
    assert_eq!(
        checked, 156,
        "all source-success writer observations must execute"
    );
}
