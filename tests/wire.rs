#[test]
fn qualified_numeric_wires() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/numeric-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = case
            .get("request_json")
            .and_then(|x| x.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| serde_json::to_string(&case["request"]).unwrap());
        assert_eq!(
            plurimath_numeric::evaluate_json(&input),
            case["expected_wire"].as_str().unwrap(),
            "case {}",
            case["id"]
        );
    }
}

#[test]
fn supplemental_numeric_wires() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/numeric-supplemental-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = serde_json::to_string(&case["request"]).unwrap();
        assert_eq!(
            plurimath_numeric::evaluate_json(&input),
            case["expected_wire"].as_str().unwrap(),
            "supplemental case {}",
            case["id"]
        );
    }
}

#[test]
fn nonfinite_recovery_wires() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/numeric-nonfinite-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = serde_json::to_string(&case["request"]).unwrap();
        assert_eq!(
            plurimath_numeric::evaluate_json(&input),
            case["expected_wire"].as_str().unwrap(),
            "nonfinite case {}",
            case["id"]
        );
    }
}

#[test]
fn bignum_identity_wires() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/numeric-bignum-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = serde_json::to_string(&case["request"]).unwrap();
        assert_eq!(
            plurimath_numeric::evaluate_json(&input),
            case["expected_wire"].as_str().unwrap(),
            "bignum case {}",
            case["id"]
        );
    }
}

#[test]
fn validation_priority_wires() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/numeric-validation-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = serde_json::to_string(&case["request"]).unwrap();
        assert_eq!(
            plurimath_numeric::evaluate_json(&input),
            case["expected_wire"].as_str().unwrap(),
            "validation case {}",
            case["id"]
        );
    }
}
