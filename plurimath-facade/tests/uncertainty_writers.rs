use plurimath_facade::request;
use serde_json::{json, Value};
#[test]
fn pma_uncertainty_all_six_source_writers() {
    let data: Value =
        serde_json::from_str(include_str!("../fixtures/uncertainty/pma27.json")).unwrap();
    let mut count = 0;
    for row in data["rows"].as_array().unwrap() {
        for (f, w) in row["writers"].as_object().unwrap() {
            let actual = request(&json!({"op":"write","model":row["model"],"format":f}));
            assert_eq!(actual["ok"], true, "{} {f}: {actual}", row["id"]);
            assert_eq!(actual["output"], w["output"], "{} {f}", row["id"]);
            count += 1;
        }
    }
    assert_eq!(count, 162);
}

#[test]
fn independent_source_fields_options_and_exceptions() {
    let data: Value =
        serde_json::from_str(include_str!("../fixtures/uncertainty/independent.json")).unwrap();
    for row in data["rows"].as_array().unwrap() {
        for (f, w) in row["writers"].as_object().unwrap() {
            let actual = request(
                &json!({"op":"write","model":row["model"],"format":f,"options":native_options(f, &row["options"])}),
            );
            if w.get("output").is_some() {
                assert_eq!(actual["ok"], true, "{} {f}: {actual}", row["id"]);
                assert_eq!(actual["output"], w["output"], "{} {f}", row["id"]);
            } else {
                assert_eq!(actual["ok"], false, "{} {f}: {actual}", row["id"]);
                assert_eq!(
                    actual["error"]["source_class"], w["error"]["class"],
                    "{} {f}: {actual}",
                    row["id"]
                );
            }
        }
    }
}

fn native_options(format: &str, source: &Value) -> Value {
    if format == "mathml" || format == "omml" {
        let mut options = source.as_object().unwrap().clone();
        if let Some(v) = options.remove("display_style") {
            options.insert("displayStyle".into(), v);
        }
        if let Some(v) = options.remove("split_on_linebreak") {
            options.insert("splitOnLinebreak".into(), v);
        }
        Value::Object(options)
    } else {
        json!({"format":source})
    }
}

#[test]
fn source_clone_validation_errors_keep_their_phase_and_message() {
    let data: Value =
        serde_json::from_str(include_str!("../fixtures/uncertainty/cloning.json")).unwrap();
    let mut calls = 0;
    for row in data["rows"].as_array().unwrap() {
        for (f, w) in row["writers"].as_object().unwrap() {
            let actual = request(
                &json!({"op":"write","model":row["model"],"format":f,"options":native_options(f,&row["options"])}),
            );
            if w.get("output").is_some() {
                assert_eq!(actual["ok"], true, "{} {f}: {actual}", row["id"]);
                assert_eq!(actual["output"], w["output"], "{} {f}", row["id"]);
            } else {
                assert_eq!(actual["ok"], false, "{} {f}: {actual}", row["id"]);
                assert_eq!(
                    actual["error"]["source_class"], w["error"]["class"],
                    "{} {f}",
                    row["id"]
                );
                assert_eq!(
                    actual["error"]["message"], w["error"]["message"],
                    "{} {f}",
                    row["id"]
                );
            }
            calls += 1;
        }
    }
    assert_eq!(calls, 38);
}
