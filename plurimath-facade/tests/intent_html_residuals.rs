use serde_json::{json, Value};

#[test]
fn source_derivative_intent_bytes() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/intent-html-residuals/source.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let actual = plurimath_facade::request(
            &json!({"op":"write","model":row["model"],"format":row["format"],"options":row["options"]}),
        );
        assert_eq!(actual["ok"], true, "{}: {actual}", row["id"]);
        assert_eq!(actual["output"], row["expected"], "{}", row["id"]);
    }
}

#[test]
fn source_html_normalization_error_and_option_precedence() {
    for (options, class) in [
        (json!({}), "Plurimath::Math::ParseError"),
        (json!({"foo":1}), "Plurimath::Math::ParseOptionError"),
        (
            json!({"locale":"xx"}),
            "Plurimath::Errors::UnsupportedLocale",
        ),
    ] {
        let actual = plurimath_facade::request(
            &json!({"op":"parse","format":"html","input":"&#55296;","options":options}),
        );
        assert_eq!(actual["ok"], false, "{actual}");
        assert_eq!(actual["error"]["source_class"], class, "{actual}");
    }
}
