use plurimath_facade::request;
use serde_json::{json, Value};

fn replay(name: &str, rows: &[Value], frozen: bool) -> (usize, usize, usize) {
    let (mut exact, mut refused, mut unported) = (0, 0, 0);
    let mut failures = vec![];
    for row in rows {
        let (id, format, model, options, expected) = if frozen {
            let c = &row["case"];
            (
                c["id"].clone(),
                row["output_format"].clone(),
                c["input"]["model"].clone(),
                c.get("options").cloned().unwrap_or(json!({})),
                if c.get("expected").is_some() {
                    json!({"output":c["expected"]})
                } else {
                    json!({"error":{"class":c["raises"]}})
                },
            )
        } else {
            (
                row["id"].clone(),
                row["format"].clone(),
                row["model"].clone(),
                row["options"].clone(),
                row["expected"].clone(),
            )
        };
        let actual =
            request(&json!({"op":"write","format":format,"model":model,"options":options}));
        if actual["ok"] == true
            && expected.get("output").is_some()
            && actual["output"] == expected["output"]
        {
            exact += 1
        } else if actual["ok"] == false
            && expected.get("error").is_some()
            && actual["error"]["source_class"] == expected["error"]["class"]
        {
            refused += 1
        } else if !frozen && actual["error"]["code"] == "unported_capability" {
            unported += 1
        } else {
            failures.push(json!({"id":id,"format":format,"actual":actual,"expected":expected}));
        }
    }
    let summary = json!({"name":name,"observations":rows.len(),"exact":exact,"source_refusal":refused,"unported":unported,"failures":failures.len()});
    println!("{summary}");
    if let Some(out) = std::env::var_os("NUMBER_DIFFERENTIAL_OUTPUT") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            std::path::Path::new(&out).join(format!("{name}.json")),
            serde_json::to_vec(&json!({"summary":summary,"failures":failures})).unwrap(),
        )
        .unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} failures: {:?}",
        failures.len(),
        failures
            .iter()
            .take(4)
            .map(|v| (&v["id"], &v["format"]))
            .collect::<Vec<_>>()
    );
    (exact, refused, unported)
}
#[test]
fn original_fourteen_source_observations() {
    let data: Value = serde_json::from_str(include_str!(
        "../fixtures/line-break-overrides/original-reference.json"
    ))
    .unwrap();
    assert_eq!(
        replay("original14", data["rows"].as_array().unwrap(), true),
        (14, 0, 0)
    );
}
#[test]
fn independent_source_shapes_and_no_split_defaults() {
    let data: Value = serde_json::from_str(include_str!(
        "../fixtures/line-break-overrides/independent-reference.json"
    ))
    .unwrap();
    let (a, b, c) = replay("independent", data["rows"].as_array().unwrap(), false);
    assert_eq!((a, b, c), (718, 254, 0));
}
