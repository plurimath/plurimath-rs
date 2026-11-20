#![cfg(feature = "xml-stages")]

use plurimath_facade::xml_parse::stage::{self, Document};
use serde_json::{json, Value};

fn cases() -> Value {
    serde_json::from_str(include_str!("../fixtures/xml-stage-smoke.json")).unwrap()
}

#[test]
fn generated_stage_documents_match_shared_ruby_xml_bytes() {
    let sample = cases();
    let rows = sample["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 11);
    for row in rows {
        let input = row["input"].as_str().unwrap();
        let format = row["format"].as_str().unwrap();
        let actual = stage::parse(input, format);
        if row["expected"].get("refusal").is_some() {
            assert!(actual.is_err(), "{} was accepted", row["id"]);
            continue;
        }
        let document = actual.unwrap_or_else(|error| panic!("{}: {error}", row["id"]));
        assert!(matches!(
            (&document, format),
            (Document::Mathml(_), "mathml") | (Document::Omml(_), "omml")
        ));
        assert_eq!(
            document.to_xml().unwrap(),
            row["expected"]["output"],
            "{}",
            row["id"]
        );
    }
}

#[test]
fn json_entrypoint_routes_each_stage_and_preserves_recorded_bytes() {
    for row in cases()["cases"].as_array().unwrap() {
        let request = json!({"op":"roundtrip","format":row["format"],"input":row["input"]});
        let response = stage::request_json(&serde_json::to_vec(&request).unwrap());
        assert_eq!(response["abi_version"], 1);
        if row["expected"].get("refusal").is_some() {
            assert_eq!(response["ok"], false, "{}", row["id"]);
            assert_eq!(response["error"]["code"], "xml_stage_parse_error");
        } else {
            assert_eq!(response["ok"], true, "{}: {response}", row["id"]);
            assert_eq!(
                response["output"], row["expected"]["output"],
                "{}",
                row["id"]
            );
        }
    }
}

#[test]
fn invalid_requests_and_unsupported_options_fail_explicitly() {
    for request in [
        json!(null),
        json!({"op":"parse","format":"mathml","input":"<math/>"}),
        json!({"op":"roundtrip","format":"latex","input":"x"}),
        json!({"op":"roundtrip","format":"mathml","input":3}),
        json!({"op":"roundtrip","format":"mathml","input":"<math/>","options":{"version":4}}),
        json!({"op":"roundtrip","format":"mathml","input":"<math/>","options":null}),
    ] {
        let response = stage::request_json(&serde_json::to_vec(&request).unwrap());
        assert_eq!(response["ok"], false, "{request}");
        assert_eq!(response["error"]["code"], "invalid_request");
    }
    assert_eq!(stage::request_json(b"{")["error"]["code"], "invalid_json");
    assert!(stage::parse("<math/>", "html").is_err());
    assert!(stage::parse("<math><mi>", "mathml").is_err());
    assert!(stage::parse("<math/>", "omml").is_err());
}

#[test]
fn stage_abi_uses_existing_facade_result_ownership() {
    let sample = cases();
    let row = &sample["cases"][0];
    let request = serde_json::to_vec(&json!({
        "op":"roundtrip","format":row["format"],"input":row["input"],"options":{}
    }))
    .unwrap();
    unsafe {
        let result = stage::plurimath_facade_xml_stage_json(request.as_ptr(), request.len());
        assert!(!result.is_null());
        let response: Value =
            serde_json::from_slice(std::ffi::CStr::from_ptr(result).to_bytes()).unwrap();
        plurimath_facade::plurimath_facade_free(result);
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(response["output"], row["expected"]["output"]);
        for len in [0, 16 * 1024 * 1024 + 1] {
            let result = stage::plurimath_facade_xml_stage_json(std::ptr::null(), len);
            let response: Value =
                serde_json::from_slice(std::ffi::CStr::from_ptr(result).to_bytes()).unwrap();
            plurimath_facade::plurimath_facade_free(result);
            assert_eq!(response["error"]["code"], "invalid_input");
        }
    }
}
