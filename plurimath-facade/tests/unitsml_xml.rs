use serde_json::{json, Value};

fn parse(input: &str) -> Value {
    plurimath_facade::request(&json!({
        "op": "parse",
        "format": "asciimath",
        "input": format!("\"unitsml({input})\""),
        "options": {},
    }))
}
fn write(model: &Value, options: Value) -> Value {
    plurimath_facade::request(&json!({
        "op": "write",
        "format": "mathml",
        "model": model,
        "options": options,
    }))
}

#[test]
fn unitsml_xml_writer_matches_source_or_stays_unported() {
    let source: Value =
        serde_json::from_str(include_str!("../fixtures/unitsml-xml/rows.json")).unwrap();
    let mut equal = vec![];
    // Source rows with xml that stay unported here, listed if the floor fails.
    let mut skipped = vec![];
    for row in source["rows"].as_array().unwrap() {
        let input = row["input"].as_str().unwrap();
        let parsed = parse(input);
        if parsed["ok"] != true {
            if row["xml"].is_string() {
                skipped.push(format!("{input}: parse {}", parsed["error"]));
            }
            continue;
        }
        let actual = write(&parsed["model"], json!({"unitsml": {"xml": true}}));
        if actual["ok"] != true {
            assert_eq!(
                actual["error"]["code"], "unported_capability",
                "{input}: {actual}"
            );
            if row["xml"].is_string() {
                skipped.push(format!("{input}: {}", actual["error"]["message"]));
            }
            continue;
        }
        assert_eq!(
            actual["output"].as_str(),
            row["xml"].as_str(),
            "{input}: native output must match the source or stay unported"
        );
        equal.push(input);
    }
    for input in [
        "m", "kg", "uF", "uH", "sr", "ksr", "rad", "Mrad", "m^2", "N*m", "m*s^-1", "km*s^-1",
        "C*kg^-1",
    ] {
        assert!(equal.contains(&input), "{input} must be ported");
    }
    // A `/` inverts the power of every unit after it, as `a*b^-1` would.
    for input in ["km/h", "km/L", "kgf*s^2/m", "h-bar/E_h"] {
        assert!(equal.contains(&input), "{input} must be ported");
    }
    assert!(
        equal.len() >= 710,
        "{} equal source rows; unported rows with source xml:\n{}",
        equal.len(),
        skipped.join("\n")
    );
}

#[test]
fn unitsml_without_xml_keeps_the_plain_row() {
    let parsed = parse("uF");
    let plain = write(&parsed["model"], json!({}));
    assert_eq!(plain["ok"], true, "{plain}");
    for options in [json!({"unitsml": {}}), json!({"unitsml": {"xml": false}})] {
        assert_eq!(write(&parsed["model"], options), plain);
    }
    let unknown = write(&parsed["model"], json!({"unitsml": {"other": true}}));
    assert_eq!(unknown["error"]["code"], "unported_capability", "{unknown}");
}
