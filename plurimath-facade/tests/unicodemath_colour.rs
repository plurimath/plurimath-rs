use serde_json::json;

fn unicodemath(input: &str) -> serde_json::Value {
    let parsed = plurimath_facade::request(
        &json!({"op":"parse","format":"asciimath","input":input,"options":{}}),
    );
    assert_eq!(parsed["ok"], true, "{input}: {parsed}");
    plurimath_facade::request(
        &json!({"op":"write","format":"unicodemath","model":parsed["model"],"options":{}}),
    )
}

#[test]
fn unicodemath_colour_names_drop_whitespace() {
    for (input, expected) in [
        ("color(red)(x)", "✎(red&x)"),
        ("color(blue)(x) + y", "✎(blue&x) + y"),
    ] {
        let written = unicodemath(input);
        assert_eq!(written["output"], expected, "{input}: {written}");
    }
}
