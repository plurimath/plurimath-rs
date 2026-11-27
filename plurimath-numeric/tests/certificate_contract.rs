//! Evaluation certificate contract for `plurimath.evaluate.gum/1` (#22).
//!
//! Two parts:
//! - completeness: every engine that runs without a device must stamp a full
//!   certificate. gum-par has its own contract with its engine work (#19), so
//!   this file covers gum-ref, the engine behind every frozen gum/1 method.
//! - additive evolution: a /1 consumer reads only the certificate members it
//!   knows. A certificate that gains a new member later must still read the
//!   same, so new fields never need a `/2` schema.

use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};

const SCHEMA: &str = "plurimath.evaluate.gum/1";

const GUM_FIXTURES: [&str; 6] = [
    include_str!("../fixtures/gum-f40.json"),
    include_str!("../fixtures/gum-annex-methods.json"),
    include_str!("../fixtures/gum-conformity.json"),
    include_str!("../fixtures/gum-mcm.json"),
    include_str!("../fixtures/gum-mcm-joint-rational.json"),
    include_str!("../fixtures/coverage-validation.json"),
];

fn run(request: &Value) -> Value {
    serde_json::from_str(&evaluate_json(&request.to_string())).unwrap()
}

fn with_provenance(mut request: Value) -> Value {
    request["parameters"]["provenance"] = json!(true);
    request
}

fn label(index: usize, request: &Value) -> String {
    format!(
        "case {index} method {}",
        request["method"].as_str().unwrap_or("?")
    )
}

/// Every gum/1 fixture request that gum-ref answers with an object result,
/// as `(request, plain response)`.
fn gum_ref_cases() -> Vec<(Value, Value)> {
    let mut found = Vec::new();
    for source in GUM_FIXTURES {
        let fixture: Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let request = &case["request"];
            if request["schema"] != SCHEMA || request["parameters"]["engine"] == "gum-par" {
                continue;
            }
            let plain = run(request);
            if plain["ok"] == true && plain["result"].is_object() {
                found.push((request.clone(), plain));
            }
        }
    }
    found
}

/// PM 1004 accuracy classes: `exact`, `limited`, `limited_float` or
/// `limited_<guard digits>`.
fn is_accuracy_class(class: &str) -> bool {
    match class {
        "exact" | "limited" | "limited_float" => true,
        other => other
            .strip_prefix("limited_")
            .is_some_and(|digits| !digits.is_empty() && digits.parse::<u64>().is_ok()),
    }
}

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    keys
}

/// Completeness contract for gum-ref: every member of the certificate is
/// present and filled as far as the engine can know it.
#[test]
fn gum_ref_certificate_is_complete_for_every_method() {
    let cases = gum_ref_cases();
    assert!(cases.len() > 50, "only {} cases found", cases.len());
    let (mut sampled, mut deterministic) = (0, 0);
    for (index, (request, plain)) in cases.iter().enumerate() {
        let label = label(index, request);
        let on = run(&with_provenance(request.clone()));
        assert_eq!(on["ok"], true, "{label}: {on}");
        assert_eq!(on["schema"], SCHEMA, "{label}");
        let provenance = &on["result"]["engine"]["provenance"];
        assert_eq!(
            sorted_keys(provenance),
            ["accuracy", "device", "engine_id", "reduction", "rng"],
            "{label}: {provenance}"
        );
        assert_eq!(provenance["engine_id"], "gum-ref", "{label}");
        // gum-ref is sequential and runs on the CPU.
        assert!(provenance["reduction"].is_null(), "{label}: {provenance}");
        assert!(provenance["device"].is_null(), "{label}: {provenance}");
        let accuracy = provenance["accuracy"]
            .as_object()
            .unwrap_or_else(|| panic!("{label}: accuracy is not a map: {provenance}"));
        assert!(!accuracy.is_empty(), "{label}: empty accuracy map");
        for (field, class) in accuracy {
            let class = class.as_str().unwrap_or("");
            assert!(
                is_accuracy_class(class),
                "{label}: accuracy class of {field} is {class:?}"
            );
        }
        let stamp = &plain["result"]["engine"];
        if stamp.get("generator").is_some() {
            sampled += 1;
            let rng = &provenance["rng"];
            assert_eq!(
                sorted_keys(rng),
                ["family", "final_state", "initial_seeds", "uniform_draws"],
                "{label}: {rng}"
            );
            assert_eq!(rng["family"], stamp["generator"], "{label}");
            assert_eq!(rng["initial_seeds"], stamp["initial_seeds"], "{label}");
            assert_eq!(rng["final_state"], stamp["final_state"], "{label}");
            assert_eq!(rng["uniform_draws"], stamp["uniform_draws"], "{label}");
            assert_eq!(rng["initial_seeds"].as_array().map(Vec::len), Some(4));
            assert!(rng["uniform_draws"].is_u64(), "{label}: {rng}");
            assert_eq!(
                sorted_keys(&provenance["accuracy"]),
                [
                    "estimate",
                    "intervals",
                    "standard_uncertainty",
                    "twice_standard_errors"
                ],
                "{label}"
            );
            for class in accuracy.values() {
                assert_eq!(*class, "limited_float", "{label}");
            }
        } else {
            deterministic += 1;
            assert!(provenance["rng"].is_null(), "{label}: {provenance}");
        }
    }
    assert!(sampled > 0, "no sampled gum-ref case was exercised");
    assert!(
        deterministic > 0,
        "no deterministic gum-ref case was exercised"
    );
}

/// Naming gum-ref explicitly selects the same engine as the default, with
/// the same certificate. Any other engine name is refused, so no request can
/// reach an engine outside the certified set.
#[test]
fn explicit_gum_ref_selection_gives_the_same_certificate() {
    let sampled: Vec<Value> = gum_ref_cases()
        .into_iter()
        .filter(|(_, plain)| plain["result"]["engine"].get("generator").is_some())
        .map(|(request, _)| request)
        .collect();
    assert!(!sampled.is_empty());
    for (index, request) in sampled.iter().enumerate() {
        let label = label(index, request);
        let default = run(&with_provenance(request.clone()));
        let mut explicit = with_provenance(request.clone());
        explicit["parameters"]["engine"] = json!("gum-ref");
        assert_eq!(run(&explicit), default, "{label}");
    }
    let mut unknown = with_provenance(sampled[0].clone());
    unknown["parameters"]["engine"] = json!("gum-device");
    let response = run(&unknown);
    assert_eq!(response["ok"], false, "{response}");
    assert!(response["result"].is_null(), "{response}");
}

/// What a `/1` consumer knows about the certificate.
#[derive(Debug, PartialEq)]
struct CertificateV1 {
    engine_id: String,
    rng: Option<RngV1>,
    reduction: Option<String>,
    accuracy: Vec<(String, String)>,
    device: Option<String>,
}

#[derive(Debug, PartialEq)]
struct RngV1 {
    family: String,
    uniform_draws: u64,
}

fn optional_string(value: &Value, name: &str) -> Result<Option<String>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        other => Err(format!("{name} must be a string or null, got {other}")),
    }
}

/// A reference `/1` reader. It checks the schema, reads the certificate
/// members it knows by name, checks their types, and ignores every member it
/// does not know. This is the reading rule that lets the certificate grow
/// without a `/2` bump.
fn read_certificate_v1(bytes: &str) -> Result<CertificateV1, String> {
    let response: Value = serde_json::from_str(bytes).map_err(|e| e.to_string())?;
    if response["schema"] != SCHEMA {
        return Err(format!("not a gum/1 response: {}", response["schema"]));
    }
    if response["ok"] != true {
        return Err("response is not ok".into());
    }
    let provenance = response
        .pointer("/result/engine/provenance")
        .filter(|value| value.is_object())
        .ok_or("result has no certificate")?;
    let engine_id = provenance["engine_id"]
        .as_str()
        .ok_or("engine_id must be a string")?
        .to_string();
    let rng = match &provenance["rng"] {
        Value::Null => None,
        rng @ Value::Object(_) => Some(RngV1 {
            family: rng["family"]
                .as_str()
                .ok_or("rng.family must be a string")?
                .to_string(),
            uniform_draws: rng["uniform_draws"]
                .as_u64()
                .ok_or("rng.uniform_draws must be an unsigned integer")?,
        }),
        other => return Err(format!("rng must be an object or null, got {other}")),
    };
    let mut accuracy = Vec::new();
    for (field, class) in provenance["accuracy"]
        .as_object()
        .ok_or("accuracy must be a map")?
    {
        let class = class.as_str().ok_or("accuracy class must be a string")?;
        accuracy.push((field.clone(), class.to_string()));
    }
    Ok(CertificateV1 {
        engine_id,
        rng,
        reduction: optional_string(&provenance["reduction"], "reduction")?,
        accuracy,
        device: optional_string(&provenance["device"], "device")?,
    })
}

/// Additive evolution: give a real gum-ref certificate members that no /1
/// consumer has heard of, at every level a consumer reads. The /1 reader
/// accepts it and reads exactly what it read before.
#[test]
fn gum_1_reader_accepts_certificate_with_unknown_added_fields() {
    let cases = gum_ref_cases();
    let sampled = cases
        .iter()
        .find(|(_, plain)| plain["result"]["engine"].get("generator").is_some())
        .expect("a sampled gum-ref case");
    let deterministic = cases
        .iter()
        .find(|(_, plain)| plain["result"]["engine"].get("generator").is_none())
        .expect("a deterministic gum-ref case");
    let added = json!({"kind":"future-member","values":[1,"2",null]});
    for (index, (request, _)) in [sampled, deterministic].into_iter().enumerate() {
        let label = label(index, request);
        let bytes = evaluate_json(&with_provenance(request.clone()).to_string());
        let known = read_certificate_v1(&bytes).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(known.engine_id, "gum-ref", "{label}");
        assert!(!known.accuracy.is_empty(), "{label}");

        let mut grown: Value = serde_json::from_str(&bytes).unwrap();
        let provenance = &mut grown["result"]["engine"]["provenance"];
        provenance["future_certificate_field"] = added.clone();
        if provenance["rng"].is_object() {
            provenance["rng"]["future_rng_field"] = added.clone();
        }
        grown["result"]["engine"]["future_engine_field"] = added.clone();
        grown["result"]["future_result_field"] = added.clone();
        grown["future_response_field"] = added.clone();
        let grown_bytes = grown.to_string();
        assert_ne!(grown_bytes, bytes, "{label}: nothing was added");
        let read = read_certificate_v1(&grown_bytes)
            .unwrap_or_else(|e| panic!("{label}: /1 reader refused an added member: {e}"));
        assert_eq!(
            read, known,
            "{label}: an added member changed what /1 reads"
        );
    }

    // The reader is not vacuous: it still refuses a broken certificate.
    let bytes = evaluate_json(&with_provenance(sampled.0.clone()).to_string());
    let broken: Value = serde_json::from_str(&bytes).unwrap();
    let mut no_id = broken.clone();
    no_id["result"]["engine"]["provenance"]
        .as_object_mut()
        .unwrap()
        .remove("engine_id");
    assert!(read_certificate_v1(&no_id.to_string()).is_err());
    let mut bad_rng = broken.clone();
    bad_rng["result"]["engine"]["provenance"]["rng"] = json!("wh");
    assert!(read_certificate_v1(&bad_rng.to_string()).is_err());
    let mut other_schema = broken;
    other_schema["schema"] = json!("plurimath.evaluate.gum/2");
    assert!(read_certificate_v1(&other_schema.to_string()).is_err());
}
