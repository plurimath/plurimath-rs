//! Residual LaTeX/UnicodeMath text shapes against frozen Ruby 0a1df68 Math.parse:
//! `\lim^n` and single-value `\substack` stay unbuilt (refused), empty `{}` binary
//! operands are nil, unmasked n-ary, empty table cells and masked n-ary scripts.
use serde_json::{json, Value};

#[test]
fn residual_text_shapes_match_ruby_source_outcomes() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/text-closure-followup/residual-shapes.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    for row in rows {
        let input = &row["input"];
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":input["format"],"input":input["text"],"options":{}}),
        );
        match row["outcome"].as_str().unwrap() {
            "model" => {
                assert_eq!(actual["ok"], true, "{input}: {actual}");
                assert_eq!(actual["model"], row["parsed_model"], "{input}");
            }
            "error" => {
                assert_eq!(actual["ok"], false, "{input}: {actual}");
                assert_eq!(
                    actual["error"]["source_class"], row["parse_error"]["class"],
                    "{input}"
                );
            }
            _ => assert_eq!(actual["ok"], false, "{input}: {actual}"),
        }
    }
    assert_eq!(rows.len(), 87);
}
