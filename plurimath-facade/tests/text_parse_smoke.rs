//! A small pinned shared-corpus regression for the public Rust parse entrypoint.
use serde_json::{json, Value};

#[test]
fn pinned_text_stage_inputs_match_shared_writer_bytes() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/text-parse-smoke.json")).unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let format = row["format"].as_str().unwrap();
        let input = row["input"].as_str().unwrap();
        let model = plurimath_facade::parse(input, format)
            .unwrap_or_else(|error| panic!("{}: {error}", row["id"]));
        assert_eq!(model["class"], "Math::Formula", "{}", row["id"]);
        assert_eq!(model["fields"]["input_string"], input, "{}", row["id"]);
        for (target, expected) in row["expected"].as_object().unwrap() {
            let output = plurimath_facade::writer::write(&model, target, &json!({}))
                .unwrap_or_else(|error| panic!("{} -> {target}: {error}", row["id"]));
            assert_eq!(
                output,
                expected.as_str().unwrap(),
                "{} -> {target}",
                row["id"]
            );
        }
        if format == "unicode" {
            assert_eq!(
                plurimath_facade::parse(input, "unicodemath").unwrap(),
                model
            );
        }
    }
}

#[test]
fn unsupported_format_returns_a_structured_error() {
    let error = plurimath_facade::parse("x", "unknown").unwrap_err();
    assert_eq!(error["code"], "unported_capability");
    assert!(error["message"].as_str().unwrap().contains("unknown"));
}
