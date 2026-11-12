use plurimath_facade::request;
use serde_json::{json, Value};
fn replay(name: &str, fixture: &str, strict: bool) -> (usize, usize, usize) {
    let data: Value = serde_json::from_str(fixture).unwrap();
    let (mut exact, mut refused, mut unported) = (0, 0, 0);
    let mut failures = vec![];
    let mut statuses = vec![];
    for row in data["rows"].as_array().unwrap() {
        for (f, expected) in row["writers"].as_object().unwrap() {
            let actual = request(
                &json!({"op":"write","format":f,"model":row["model"],"options":row.get("options").cloned().unwrap_or(json!({}))}),
            );
            let status = if actual["ok"] == true
                && expected.get("output").is_some()
                && actual["output"] == expected["output"]
            {
                exact += 1;
                "exact"
            } else if actual["ok"] == false
                && expected.get("error").is_some()
                && actual["error"]["source_class"] == expected["error"]["class"]
            {
                refused += 1;
                "source_refusal"
            } else if !strict
                && actual["ok"] == false
                && actual["error"]["code"] == "unported_capability"
            {
                unported += 1;
                "unported"
            } else {
                failures
                    .push(json!({"id":row["id"],"format":f,"actual":actual,"expected":expected}));
                "different"
            };
            statuses.push(
                json!({"id":row["id"],"format":f,"status":status,"error":actual.get("error")}),
            );
        }
    }
    let summary = json!({"name":name,"cases":data["rows"].as_array().unwrap().len(),"exact":exact,"source_refusal":refused,"unported":unported,"failures":failures.len()});
    println!("{}", summary);
    if let Some(out) = std::env::var_os("NUMBER_DIFFERENTIAL_OUTPUT") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            std::path::Path::new(&out).join(format!("{name}.json")),
            serde_json::to_vec(&json!({"summary":summary,"rows":statuses,"failures":failures}))
                .unwrap(),
        )
        .unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} different observations; IDs {:?}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .map(|x| (&x["id"], &x["format"]))
            .collect::<Vec<_>>()
    );
    (exact, refused, unported)
}
#[test]
fn original_operator_number_gaps() {
    assert_eq!(
        replay(
            "operator52",
            include_str!("../fixtures/numbers/operator-number-gaps.json"),
            true
        ),
        (52, 0, 0)
    );
}
#[test]
fn source_initialization_setters_and_varied_number_values() {
    let (a, b, c) = replay(
        "numbers",
        include_str!("../fixtures/numbers/source-reference.json"),
        false,
    );
    assert_eq!((a, b, c), (2772, 243, 945));
}
