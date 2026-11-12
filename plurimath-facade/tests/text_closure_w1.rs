//! Frozen Ruby 0a1df68 Math.parse models for the LaTeX text-closure family
//! (n-ary symbols, row breaks, substack, limits, rule, scripted left/right, references).
use serde_json::{json, Value};

#[test]
fn source_latex_models_and_errors() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/text-closure-w1/latex-models.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    for row in rows {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"latex","input":row["input"]["text"],"options":{}}),
        );
        if let Some(model) = row.get("parsed_model") {
            assert_eq!(actual["ok"], true, "{}: {actual}", row["input"]["text"]);
            assert_eq!(&actual["model"], model, "{}", row["input"]["text"]);
        } else {
            assert_eq!(actual["ok"], false, "{}: {actual}", row["input"]["text"]);
            assert_eq!(
                actual["error"]["source_class"], row["parse_error"]["class"],
                "{}",
                row["input"]["text"]
            );
        }
    }
    assert_eq!(rows.len(), 37);
}
