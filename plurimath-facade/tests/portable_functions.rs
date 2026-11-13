use plurimath_facade::request;
use serde_json::{json, Value};
#[test]
fn required_portable_output_and_error_gaps_match_source() {
    let rows: Value = serde_json::from_str(include_str!(
        "../fixtures/portable-functions/required46.json"
    ))
    .unwrap();
    let (mut output, mut classes) = (0, 0);
    for row in rows.as_array().unwrap() {
        let actual = request(
            &json!({"op":"write","model":row["model"],"format":row["format"],"options":row["options"]}),
        );
        if row["expected"].get("output").is_some() {
            assert_eq!(actual["ok"], true, "{actual}");
            assert_eq!(
                actual["output"], row["expected"]["output"],
                "{} {}",
                row["id"], row["format"]
            );
            output += 1;
        } else {
            assert_eq!(actual["ok"], false);
            assert_eq!(
                actual["error"]["source_class"], row["expected"]["error"]["class_"],
                "{}",
                row["id"]
            );
            classes += 1;
        }
    }
    assert_eq!((output, classes), (37, 9));
}
#[test]
fn independent_unary_binary_fields_preserve_source_outputs_and_refusals() {
    let source: Value = serde_json::from_str(include_str!(
        "../fixtures/portable-functions/independent456.json"
    ))
    .unwrap();
    let (mut output, mut refused) = (0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            let actual = request(&json!({"op":"write","model":row["model"],"format":format}));
            if expected.get("output").is_some() {
                assert_eq!(actual["ok"], true, "{} {format}: {actual}", row["id"]);
                assert_eq!(
                    actual["output"], expected["output"],
                    "{} {format}",
                    row["id"]
                );
                output += 1;
            } else {
                assert_eq!(actual["ok"], false, "{} {format}", row["id"]);
                assert_eq!(
                    actual["error"]["source_class"], expected["error"]["class"],
                    "{} {format}",
                    row["id"]
                );
                refused += 1;
            }
        }
    }
    assert_eq!((output, refused), (1004, 1732));
}
