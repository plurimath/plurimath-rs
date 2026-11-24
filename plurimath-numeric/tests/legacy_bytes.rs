//! Byte-level regression gate for the scalar-tower work (issue #16).
//!
//! `fixtures/legacy-bytes.json` holds requests taken from the frozen GUM
//! fixtures together with the exact output string `evaluate_json` produced
//! before the Scalar abstraction was introduced. Unlike `tests/gum.rs`, the
//! comparison is on raw bytes, not parsed JSON.
use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};

const IDS: &[&str] = &[
    "producer-01",
    "producer-02",
    "producer-00",
    "producer-03",
    "producer-05",
    "producer-06",
    "refusals-10",
    "coverage-00",
    "h6-resolution-and-unknown-dof",
    "h4-ratio-approach",
];

#[test]
fn output_bytes_match_recorded_legacy_output() {
    let source = include_str!("../fixtures/legacy-bytes.json");
    let recorded: Value = serde_json::from_str(source).unwrap();
    let entries = recorded.as_array().unwrap();
    assert_eq!(entries.len(), IDS.len());
    for entry in entries {
        let request = entry["request"].to_string();
        let actual = evaluate_json(&request);
        assert_eq!(
            actual,
            entry["output"].as_str().unwrap(),
            "bytes changed for {}",
            entry["id"]
        );
    }
}

/// Regenerates the fixture from the frozen cases. Run only against the legacy
/// code: `BLESS_LEGACY_BYTES=1 cargo test -p plurimath-numeric --test legacy_bytes bless`.
#[test]
fn bless() {
    if std::env::var_os("BLESS_LEGACY_BYTES").is_none() {
        return;
    }
    let mut cases = Vec::new();
    for file in [
        include_str!("../fixtures/gum-f40.json"),
        include_str!("../fixtures/gum-conformity.json"),
        include_str!("../fixtures/gum-annex-methods.json"),
    ] {
        let fixture: Value = serde_json::from_str(file).unwrap();
        cases.extend(fixture["cases"].as_array().unwrap().clone());
    }
    let entries: Vec<Value> = IDS
        .iter()
        .map(|id| {
            let case = cases.iter().find(|c| c["id"] == *id).unwrap();
            let request = case["request"].clone();
            let output = evaluate_json(&request.to_string());
            json!({"id": id, "request": request, "output": output})
        })
        .collect();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/legacy-bytes.json");
    std::fs::write(path, serde_json::to_string_pretty(&entries).unwrap() + "\n").unwrap();
}
