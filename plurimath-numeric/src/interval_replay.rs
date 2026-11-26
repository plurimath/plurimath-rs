//! Replay of the frozen coverage-validation wires against the rigorous
//! LPU-adequacy verdict (issue #18, "agreement with the MCM adequacy
//! fixtures").
//!
//! Fixture: `fixtures/gum-v1-coverage-validation-wires.json`, copied
//! byte-for-byte from plurimath-testsuite `evaluation/gum-v1/` (testsuite
//! commit 4d3f6328540578c58212e675553d68441c13e716).
//! Producer per that directory's manifest.json: plurimath-numeric revision
//! 8f9273968a9e17dc604eb1ae66a84b9c4fa30f1c; file sha256
//! 9d1335826628737976c927715c27225003ba501061a3397e9069c7b9519cbc7b
//! (49 cases).
//!
//! The wires carry one model: the case `validation-source-normal-170k`
//! (`y = x`, `x ~ N(3, 2)`, the source of the 170000-sample MCM run that every
//! `validate_coverage` case reuses through `variables: ["x"]`). That model is
//! replayed through `lpu_adequacy` with coverage factor 1.96 (probability
//! 0.95), threshold 0.05 and a fixed subdivision budget.
//!
//! Agreement rule. The MCM check compares interval endpoints within a
//! tolerance; the interval check bounds the linearization error. They are not
//! 1:1, so only two things are asserted:
//!
//! - `inadequate` implies `not_validated`;
//! - `adequate` must not contradict `validated`.
//!
//! `adequate` against `not_validated` is not a contradiction (the MCM
//! intervals of those cases are perturbed on purpose) and is counted
//! separately. `inconclusive` is counted separately and never forced to a
//! verdict. Refusal wires and wires that carry no adequacy symbol are not
//! replayed and are counted as skipped.

use crate::interval::{lpu_adequacy, Adequacy, IDual, Verdict};
use crate::E;
use serde_json::Value;

const FIXTURE: &str = include_str!("../fixtures/gum-v1-coverage-validation-wires.json");
const COVERAGE_FACTOR_95: f64 = 1.96;
const THRESHOLD: f64 = 0.05;
/// Subdivision budget (maximum number of boxes) given to every replay.
const BUDGET: usize = 256;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// adequate and validated, or inadequate and not_validated
    Agree,
    /// adequate while the MCM wire says not_validated (not a contradiction)
    AdequateNotValidated,
    /// the interval verdict could not be decided within the budget
    Inconclusive,
    /// inadequate while validated
    Contradiction,
}

fn classify(verdict: Verdict, expected: &str) -> Outcome {
    match (verdict, expected) {
        (Verdict::Inconclusive, _) => Outcome::Inconclusive,
        (Verdict::Adequate, "validated") | (Verdict::Inadequate, "not_validated") => Outcome::Agree,
        (Verdict::Adequate, "not_validated") => Outcome::AdequateNotValidated,
        _ => Outcome::Contradiction,
    }
}

struct Model {
    centre: f64,
    sd: f64,
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// The model of the single `monte_carlo` wire: a bare variable with a normal
/// input. Anything else would need an AST interpreter and is not mapped.
fn source_model(cases: &[Value]) -> Model {
    let case = cases
        .iter()
        .find(|c| c["id"] == "validation-source-normal-170k")
        .expect("source model case");
    let request = parse(case["request_json"].as_str().unwrap());
    let p = &request["parameters"];
    assert_eq!(p["ast"]["type"], "variable");
    let input = &p["inputs"][0];
    assert_eq!(input["name"], p["ast"]["name"]);
    assert_eq!(input["pdf"]["distribution"]["symbol"], "normal");
    Model {
        centre: input["pdf"]["location"].as_f64().unwrap(),
        sd: input["pdf"]["sd"].as_f64().unwrap(),
    }
}

fn identity(x: &[IDual]) -> Result<IDual, E> {
    Ok(x[0])
}

fn replay(model: &Model) -> Adequacy {
    lpu_adequacy(
        &identity,
        &[model.centre],
        &[model.sd],
        COVERAGE_FACTOR_95,
        THRESHOLD,
        BUDGET,
    )
    .unwrap()
}

#[derive(Default, Debug)]
struct Counts {
    agree: usize,
    adequate_not_validated: usize,
    inconclusive: usize,
    contradiction: usize,
    skipped: usize,
}

#[test]
fn lpu_adequacy_agrees_with_coverage_validation_wires() {
    let fixture = parse(FIXTURE);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 49);
    let model = source_model(cases);
    let mut counts = Counts::default();
    let mut failures = Vec::new();
    for case in cases {
        let wire = parse(case["expected_wire"].as_str().unwrap());
        let symbol = wire["result"]["adequacy"]["symbol"].as_str();
        let (true, Some(symbol)) = (wire["ok"] == true, symbol) else {
            counts.skipped += 1;
            continue;
        };
        let r = replay(&model);
        // The budget is part of the result and bounds the work done.
        assert_eq!(r.budget, BUDGET);
        assert!(r.boxes_used >= 1 && r.boxes_used <= r.budget, "{r:?}");
        match classify(r.verdict, symbol) {
            Outcome::Agree => counts.agree += 1,
            Outcome::AdequateNotValidated => counts.adequate_not_validated += 1,
            Outcome::Inconclusive => counts.inconclusive += 1,
            Outcome::Contradiction => {
                counts.contradiction += 1;
                failures.push(format!(
                    "{}: {} against {symbol} {}",
                    case["id"],
                    r.verdict.as_str(),
                    r.to_json()
                ));
            }
        }
    }
    eprintln!(
        "lpu_adequacy replay: {counts:?}, budget {BUDGET}, model y=x, c={}, u={}",
        model.centre, model.sd
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // 14 validated + 3 not_validated wires carry an adequacy symbol; 32 are
    // refusals or carry none.
    assert_eq!(
        counts.agree + counts.adequate_not_validated + counts.inconclusive,
        17,
        "{counts:?}"
    );
    assert_eq!(counts.skipped, 32, "{counts:?}");
}

#[test]
fn replay_records_budget_and_stays_proven_for_a_linear_model() {
    let r = replay(&Model {
        centre: 3.0,
        sd: 2.0,
    });
    assert_eq!(r.verdict, Verdict::Adequate);
    assert_eq!(r.budget, BUDGET);
    assert_eq!(r.to_json()["subdivision"]["budget"], BUDGET);
}

#[test]
fn classification_never_forces_inconclusive() {
    assert_eq!(classify(Verdict::Adequate, "validated"), Outcome::Agree);
    assert_eq!(
        classify(Verdict::Inadequate, "not_validated"),
        Outcome::Agree
    );
    assert_eq!(
        classify(Verdict::Adequate, "not_validated"),
        Outcome::AdequateNotValidated
    );
    assert_eq!(
        classify(Verdict::Inadequate, "validated"),
        Outcome::Contradiction
    );
    for symbol in ["validated", "not_validated"] {
        assert_eq!(
            classify(Verdict::Inconclusive, symbol),
            Outcome::Inconclusive
        );
    }
}
