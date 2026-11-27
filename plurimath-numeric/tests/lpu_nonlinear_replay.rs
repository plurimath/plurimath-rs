//! Replay of the frozen nonlinear `lpu_adequacy` wires (issue #18).
//!
//! Fixture: `fixtures/gum-v1-lpu-adequacy-nonlinear-wires.json`, copied
//! byte-for-byte from plurimath-testsuite `evaluation/lpu-adequacy-v1/`.
//! Each row is an `lpu_adequacy` request, the exact reply of this crate, and
//! an independent Monte Carlo (MCM) coverage check of the same model
//! (`mcm.symbol`: `validated` / `not_validated`).
//!
//! Two assertions per row:
//!
//! - the reply bytes equal `expected_wire`;
//! - the verdict agrees with MCM: `inadequate` implies `not_validated`;
//!   `adequate` implies `validated`; `inconclusive` is never forced.

use serde_json::{json, Value};

const FIXTURE: &str = include_str!("../fixtures/gum-v1-lpu-adequacy-nonlinear-wires.json");

#[derive(Debug, Default)]
struct Counts {
    adequate: usize,
    inadequate: usize,
    inconclusive: usize,
}

#[test]
fn nonlinear_lpu_adequacy_wires_replay_and_agree_with_mcm() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    // Draft mode: rows without a frozen wire are printed for freezing.
    let draft: Vec<&Value> = cases.iter().filter(|c| c["expected_wire"] == "").collect();
    if !draft.is_empty() {
        let mut wires = serde_json::Map::new();
        for case in &draft {
            let wire = plurimath_numeric::evaluate_json(case["request_json"].as_str().unwrap());
            wires.insert(case["id"].as_str().unwrap().to_string(), json!(wire));
        }
        panic!("UNFROZEN-WIRES {}", Value::Object(wires));
    }
    let mut counts = Counts::default();
    let mut failures = Vec::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let wire = plurimath_numeric::evaluate_json(case["request_json"].as_str().unwrap());
        if wire != case["expected_wire"].as_str().unwrap() {
            failures.push(format!("{id}: wire differs: {wire}"));
            continue;
        }
        let parsed: Value = serde_json::from_str(&wire).unwrap();
        let verdict = parsed["result"]["verdict"].as_str().unwrap();
        let mcm = case["mcm"]["symbol"].as_str().unwrap();
        match (verdict, mcm) {
            ("adequate", "validated") => counts.adequate += 1,
            ("inadequate", "not_validated") => counts.inadequate += 1,
            ("inconclusive", _) => counts.inconclusive += 1,
            _ => failures.push(format!("{id}: {verdict} against MCM {mcm}")),
        }
    }
    eprintln!("lpu_adequacy nonlinear replay: {counts:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // A non-adequate verdict must match MCM, not only adequate ones.
    assert_eq!(counts.adequate, 3, "{counts:?}");
    assert_eq!(counts.inadequate, 4, "{counts:?}");
    assert_eq!(counts.inconclusive, 1, "{counts:?}");
}
