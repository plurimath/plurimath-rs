//! Frozen observations of Ruby Math.parse. No decimal replacement or model normalization.
use serde_json::{json, Value};

#[test]
fn source_html_locale_models_and_errors() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/parse-locales/source-models.json")).unwrap();
    for row in fixture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["input"]["format"] == "html")
    {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"html","input":row["input"]["text"],"options":row["parseOptions"]}),
        );
        if row.get("parsed_model").is_some() {
            assert_eq!(actual["ok"], true, "{}: {actual}", row["id"]);
            assert_eq!(actual["model"], row["parsed_model"], "{}", row["id"]);
        } else {
            assert_eq!(actual["ok"], false, "{}: {actual}", row["id"]);
            assert_eq!(
                actual["error"]["source_class"], row["parse_error"]["class"],
                "{}",
                row["id"]
            );
            assert_eq!(
                actual["error"]["message"], row["parse_error"]["message"],
                "{}",
                row["id"]
            );
        }
    }
}
