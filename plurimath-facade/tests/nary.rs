use plurimath_facade::request;
use serde_json::{json, Value};
fn replay(name: &str, fixture: &str, strict: bool) -> (usize, usize, usize) {
    let source: Value = serde_json::from_str(fixture).unwrap();
    let mut rows = vec![];
    let (mut outputs, mut refusals, mut unported) = (0, 0, 0);
    let mut failures = vec![];
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            let options = row
                .get("writer_options")
                .and_then(|o| o.get("options"))
                .cloned()
                .unwrap_or(json!({}));
            let actual = request(
                &json!({"op":"write","model":row["model"],"format":format,"options":options}),
            );
            let status = if expected.get("output").is_some()
                && actual["ok"] == true
                && actual["output"] == expected["output"]
            {
                outputs += 1;
                "exact_output"
            } else if expected.get("error").is_some()
                && actual["ok"] == false
                && actual["error"]["source_class"] == expected["error"]["class"]
            {
                refusals += 1;
                "exact_source_refusal_class"
            } else if !strict
                && actual["ok"] == false
                && actual["error"]["code"] == "unported_capability"
            {
                unported += 1;
                "unported"
            } else {
                failures.push(
                    json!({"id":row["id"],"format":format,"actual":actual,"expected":expected}),
                );
                "different"
            };
            rows.push(
                json!({"id":row["id"],"format":format,"status":status,"error":actual.get("error")}),
            );
        }
    }
    let summary = json!({"name":name,"cases":source["rows"].as_array().unwrap().len(),"exact_outputs":outputs,"source_refusal_classes":refusals,"unported":unported,"failures":failures.len()});
    println!("{}", summary);
    if let Some(out) = std::env::var_os("NARY_DIFFERENTIAL_OUTPUT") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            std::path::Path::new(&out).join(format!("{name}.json")),
            serde_json::to_vec(&json!({"summary":summary,"rows":rows,"failures":failures}))
                .unwrap(),
        )
        .unwrap();
    }
    assert!(
        failures.is_empty(),
        "{name}: {} mismatches; bounded IDs {:?}",
        failures.len(),
        failures
            .iter()
            .take(6)
            .map(|v| (&v["id"], &v["format"]))
            .collect::<Vec<_>>()
    );
    (outputs, refusals, unported)
}
#[test]
fn frozen_source_nary_writers() {
    assert_eq!(
        replay(
            "frozen",
            include_str!("../fixtures/nary/source-reference.json"),
            true
        ),
        (836, 316, 0)
    );
}
#[test]
fn independent_shapes_mask_bits_and_source_refusals() {
    let (o, e, u) = replay(
        "independent",
        include_str!("../fixtures/nary/independent-reference.json"),
        false,
    );
    assert_eq!(o + e + u, 1728);
}
