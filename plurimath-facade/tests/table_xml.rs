use plurimath_facade::writer;
use serde_json::{json, Value};
#[test]
fn source_table_xml_shapes_and_refusals() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/table-xml-source.json")).unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        for (format, expected) in row["outputs"].as_object().unwrap() {
            let actual = writer::write(&row["model"], format, &json!({}));
            if let Some(expected) = expected.as_str() {
                assert_eq!(actual.unwrap(), expected, "{} {}", row["id"], format);
            } else {
                assert!(actual.is_err(), "source refusal {} {}", row["id"], format);
            }
        }
    }
}

#[test]
fn inherited_table_subclass_writers_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/table-subclass-writers-source.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 7);
    let mut observations = 0;
    let mut differences = Vec::new();
    for row in rows {
        for (format, expected) in row["writers"].as_object().unwrap() {
            let (format, options) = if format == "mathml_intent" {
                ("mathml", json!({"intent":true}))
            } else {
                (format.as_str(), json!({}))
            };
            let actual = writer::write(&row["model"], format, &options);
            if actual.as_deref() != Ok(expected["value"].as_str().unwrap()) {
                differences.push(format!("{} {format}: {actual:?}", row["id"]));
            }
            observations += 1;
        }
    }
    assert_eq!(observations, 49);
    assert!(differences.is_empty(), "{differences:#?}");
}

#[test]
fn cases_intent_uses_the_explicit_ruby0a1_source_profile() {
    let profile: Value = serde_json::from_str(include_str!(
        "../fixtures/source-profile-ruby0a1-cases.json"
    ))
    .unwrap();
    let rows = profile["corrections"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    for row in rows {
        let parsed = plurimath_facade::request(&json!({
            "op":"parse", "format":row["input"]["format"],
            "input":row["input"]["text"], "options":{}
        }));
        assert_eq!(parsed["ok"], true, "{parsed}");
        let output = plurimath_facade::request(&json!({
            "op":"write", "format":row["format"],
            "model":parsed["model"], "options":row["options"]
        }));
        assert_eq!(output["ok"], true, "{output}");
        assert_eq!(output["output"], row["expected_output"]);
    }
}
