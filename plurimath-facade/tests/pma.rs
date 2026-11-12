use serde_json::{json, Value};
#[test]
fn actual_source_pma_models_and_refusals() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/pma/ruby-reference.json")).unwrap();
    let (mut accepted, mut refused) = (0, 0);
    for row in corpus["rows"].as_array().unwrap() {
        let got = plurimath_facade::request_json(
            &serde_json::to_vec(&json!({"op":"parse","format":"pma","input":row["input"]}))
                .unwrap(),
        );
        if let Some(want) = row.get("model") {
            assert_eq!(got["ok"], true, "{}: {}", row["id"], got);
            assert_eq!(&got["model"], want, "{}", row["id"]);
            accepted += 1;
        } else {
            assert_eq!(got["ok"], false, "{}: {}", row["id"], got);
            assert_eq!(
                got["error"]["source_class"], "Plurimath::Math::ParseError",
                "{}: {}",
                row["id"], got
            );
            refused += 1;
        }
    }
    assert_eq!((accepted, refused), (34, 64));
}

#[test]
fn source_nested_units_parentheses_retain_all_six_writers() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/pma/ruby-reference.json")).unwrap();
    let row = corpus["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "unit.nested_depth_8")
        .unwrap();
    let got = plurimath_facade::request_json(
        &serde_json::to_vec(&json!({"op":"parse","format":"pma","input":row["input"]})).unwrap(),
    );
    assert_eq!(got["ok"], true);
    for format in [
        "asciimath",
        "latex",
        "unicodemath",
        "mathml",
        "omml",
        "html",
    ] {
        let result = plurimath_facade::writer::write(&got["model"], format, &json!({})).unwrap();
        assert_eq!(
            result,
            row["writers"][format]["output"].as_str().unwrap(),
            "{format}"
        );
    }
}
