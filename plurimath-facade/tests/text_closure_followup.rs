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
    let mut differences = Vec::new();
    for row in rows {
        let input = &row["input"];
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":input["format"],"input":input["text"],"options":{}}),
        );
        let exact = match row["outcome"].as_str().unwrap() {
            "model" => actual["ok"] == true && actual["model"] == row["parsed_model"],
            "error" => {
                actual["ok"] == false
                    && actual["error"]["source_class"] == row["parse_error"]["class"]
            }
            _ => actual["ok"] == false,
        };
        if !exact {
            differences.push(json!({"input":input,"actual":actual,"expected":row}));
        }
    }
    assert_eq!(rows.len(), 87);
    assert!(
        differences.is_empty(),
        "{} source differences: {differences:#?}",
        differences.len()
    );
}
