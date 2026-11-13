//! UnicodeMath (and LaTeX infimum) capture -> common-model rows measured
//! against Ruby plurimath 0a1df68 (fixtures/text-unicode-captures/README note
//! is in the fixture's `source` field). Rows whose Ruby result is an unbuilt
//! capture Hash or a source error must stay refused.
use serde_json::{json, Value};

#[test]
fn unicode_capture_rows_match_ruby_source_models() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/text-unicode-captures/source-models.json"
    ))
    .unwrap();
    let (mut equal, mut refused) = (0, 0);
    for row in fixture["rows"].as_array().unwrap() {
        let response = plurimath_facade::request(
            &json!({"op":"parse","format":row["format"],"input":row["text"]}),
        );
        match row["expect"].as_str().unwrap() {
            "model" => {
                assert_eq!(response["ok"], true, "{}: {response}", row["text"]);
                assert_eq!(response["model"], row["parsed_model"], "{}", row["text"]);
                equal += 1;
            }
            "refuse" => {
                assert_eq!(response["ok"], false, "{}: {response}", row["text"]);
                refused += 1;
            }
            other => panic!("unknown expectation {other}"),
        }
    }
    assert_eq!(
        (equal, refused),
        (
            fixture["counts"]["model"].as_u64().unwrap() as usize,
            fixture["counts"]["refuse"].as_u64().unwrap() as usize
        )
    );
}
