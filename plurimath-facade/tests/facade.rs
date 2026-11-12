use plurimath_facade::{model::Model, parser, writer};
use serde_json::{json, Value};
#[test]
fn actual_source_ten_parse_and_sixty_writers() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/native-slice-ruby.json")).unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        let model = parser::parse(
            row["input"].as_str().unwrap(),
            row["format"].as_str().unwrap(),
            &json!({}),
        )
        .unwrap();
        assert_eq!(model, row["model"], "common model {}", row["id"]);
        for (fmt, expected) in row["writers"].as_object().unwrap() {
            assert_eq!(
                writer::write(&model, fmt, &json!({})).unwrap(),
                expected.as_str().unwrap(),
                "writer {} {fmt}",
                row["id"]
            );
        }
    }
}
#[test]
fn source_mixed_field_shape_is_preserved() {
    let v = json!({"class":"Math::Formula","fields":{"value":[null,"plain",{"class":"Math::Number","fields":{"value":"2","base":null}}],"options":{"symbol":{"string":"x"},"empty":[]},"input_string":"a\u{0}b"}});
    assert_eq!(Model::from_value(&v).unwrap().value(), v);
    assert!(writer::write(&v, "asciimath", &json!({})).is_err());
}
#[test]
fn remaining_promises_refuse_and_completed_nary_matches_source() {
    assert!(parser::parse("x", "pma", &json!({}))
        .unwrap_err()
        .to_string()
        .contains("unported promised"));
    let source: Value = serde_json::from_str(include_str!("../fixtures/generic-nary/generic-nary-source.json")).unwrap();
    let row = source["rows"].as_array().unwrap().iter().find(|row| row["id"] == "slot1-nil").unwrap();
    assert_eq!(writer::write(&row["model"], "latex", &json!({})).unwrap(), row["writers"]["latex"]["value"].as_str().unwrap());
}
#[test]
fn abi_ownership_and_invalid_input() {
    unsafe {
        let s = b"{";
        let p = plurimath_facade::plurimath_facade_json(s.as_ptr(), s.len());
        assert!(!p.is_null());
        let v: Value = serde_json::from_slice(std::ffi::CStr::from_ptr(p).to_bytes()).unwrap();
        assert_eq!(v["error"]["code"], "invalid_json");
        plurimath_facade::plurimath_facade_free(p);
        plurimath_facade::plurimath_facade_free(std::ptr::null_mut());
    }
}
