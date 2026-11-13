use serde_json::{json, Value};
#[test]
fn actual_ruby_source_ascii_closure() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/ascii-source-closure/reference.json"
    ))
    .unwrap();
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 98);
    for case in rows {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"asciimath","input":case["input"]["text"],"options":case["parseOptions"]}),
        );
        if case.get("parsed_model").is_some() {
            assert_eq!(
                actual["model"], case["parsed_model"],
                "source row {}: {}",
                case["index"], actual
            );
        } else {
            assert_eq!(actual["ok"], false, "source row {}", case["index"]);
            let expected_class = case["parse_error"]["class"].as_str().unwrap();
            if expected_class == "Plurimath::Math::ParseError" {
                assert_eq!(
                    actual["error"]["code"], "syntax_error",
                    "source row {}: {}",
                    case["index"], actual
                );
            } else {
                assert_eq!(
                    actual["error"]["source_class"], expected_class,
                    "source row {}: {}",
                    case["index"], actual
                );
                assert_eq!(
                    actual["error"]["message"], case["parse_error"]["message"],
                    "source error message row {}",
                    case["index"]
                );
            }
        }
    }
}
