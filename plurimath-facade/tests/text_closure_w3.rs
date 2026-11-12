//! LaTeX table environments, alignment/separator/horizontal-line options and `nil`
//! captures against the actual Ruby 0a1 source models.
use serde_json::json;

#[test]
fn latex_tables_and_nil_captures_match_ruby_source_models() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/text-closure-w3/latex-tables-nil.json"
    ))
    .unwrap();
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 18);
    for row in rows {
        let input = &row["input"];
        let reply = plurimath_facade::request(
            &json!({"op":"parse","format":input["format"],"input":input["text"]}),
        );
        assert_eq!(reply["ok"], true, "{input}: {reply}");
        assert_eq!(reply["model"], row["parsed_model"], "{input}");
    }
}
