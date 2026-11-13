use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};

const PROFILE: &str = "plurimath.evaluate.quantity/1";

fn run(request: Value) -> Value {
    serde_json::from_str(&evaluate_json(&request.to_string())).unwrap()
}

#[cfg(feature = "quantity")]
mod native {
    use super::*;
    use std::collections::BTreeMap;

    fn variable(name: &str) -> Value {
        json!({"type":"variable","name":name})
    }

    fn number(text: &str) -> Value {
        json!({"type":"number","text":text})
    }

    fn binary(operator: &str, left: Value, right: Value) -> Value {
        json!({"type":"binary","operator":operator,"left":left,"right":right})
    }

    fn request(ast: Value, bindings: Value) -> Value {
        json!({"schema":PROFILE,"ast":ast,"bindings":bindings})
    }

    #[test]
    fn frozen_f40_voltage_current_conversion() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/quantity-f40.json")).unwrap();
        let response = run(fixture["request"].clone());
        assert_eq!(response["ok"], true, "{response}");
        let expected = &fixture["expected"];
        let result = &response["result"];
        assert_eq!(result["accuracy"], expected["outcome"]["accuracy"]);
        for (actual, expected) in [
            (&result["value"], &expected["outcome"]),
            (&result["converted"], &expected["converted"]),
        ] {
            let pair = &expected["magnitude"]["rational"];
            assert_eq!(pair[1], "1");
            assert_eq!(actual["magnitude"], pair[0]);
        }
        let dims: BTreeMap<_, _> = expected["outcome"]["dimensions"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, power)| (name.clone(), power.to_string()))
            .collect();
        assert_eq!(result["value"]["dimensions"], json!(dims));
        assert_eq!(result["converted"]["dimensions"], json!(dims));
        assert_eq!(result["value"]["canonical_magnitude"], "6000");
        assert_eq!(result["converted"]["canonical_magnitude"], "6000");
        assert_eq!(result["value"]["scale"], "ratio");
        assert_eq!(result["converted"]["scale"], "ratio");
        assert_eq!(result["value"]["dimension_id"], "NISTd16");
        let mut volts = fixture["request"].clone();
        volts["bindings"]["v"] = json!({"value":"2000","unit":"V"});
        let volts = run(volts);
        assert_eq!(volts["result"]["value"]["magnitude"], "6000");
        assert_eq!(volts["result"]["converted"]["magnitude"], "6");
        assert_eq!(
            result["converted"]["terms"],
            json!([{"unit":"NISTu14","prefix":"NISTp10_3","power":"1"}])
        );
    }

    #[test]
    fn ordinary_ast_keeps_units_in_nested_intermediates() {
        let bindings = json!({"x":{"value":"1","unit":"km"},"y":{"value":"1","unit":"m"},"t":{"value":"2","unit":"s"}});
        let sum = binary("+", variable("x"), variable("y"));
        let mut grouped = request(binary("/", sum, variable("t")), bindings.clone());
        grouped["convert_to"] = json!("m/s");
        let result = run(grouped);
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["result"]["value"]["magnitude"], "1001/2000");
        assert_eq!(result["result"]["converted"]["magnitude"], "1001/2");
        assert_eq!(
            result["result"]["converted"]["dimensions"],
            json!({"Length":"1","Time":"-1"})
        );
        let ungrouped = run(request(
            binary(
                "+",
                variable("x"),
                binary("/", variable("y"), variable("t")),
            ),
            bindings,
        ));
        assert_eq!(ungrouped["error"]["code"], "DimensionMismatchError");
    }

    #[test]
    fn affine_difference_and_scale_gates() {
        let bindings = json!({"a":{"value":"20","unit":"degC"},"b":{"value":"15","unit":"degC"}});
        let mut difference = request(binary("-", variable("a"), variable("b")), bindings.clone());
        difference["convert_to"] = json!("K");
        let result = run(difference);
        assert_eq!(result["result"]["converted"]["magnitude"], "5");
        assert_eq!(result["result"]["converted"]["scale"], "difference");
        let absolute_plus_difference = run(request(
            binary("+", variable("a"), variable("delta")),
            json!({"a":{"value":"20","unit":"degC"},"delta":{"value":"5","unit":"K","difference":true}}),
        ));
        assert_eq!(
            absolute_plus_difference["result"]["value"]["magnitude"],
            "25"
        );
        assert_eq!(
            absolute_plus_difference["result"]["value"]["scale"],
            "interval"
        );
        let interval = run(request(binary("*", variable("a"), number("2")), bindings));
        assert_eq!(interval["error"]["code"], "ScaleOperationError");
        assert_eq!(interval["error"]["class"], "error");
    }

    #[test]
    fn integer_powers_and_typed_refusals() {
        let bindings = json!({"x":{"value":"2","unit":"km"}});
        let mut reciprocal = request(binary("^", variable("x"), number("-2")), bindings.clone());
        reciprocal["convert_to"] = json!("m^-2");
        let result = run(reciprocal);
        assert_eq!(result["result"]["value"]["magnitude"], "1/4");
        assert_eq!(result["result"]["converted"]["magnitude"], "1/4000000");
        let dimensionful = run(request(binary("^", number("2"), variable("x")), bindings));
        assert_eq!(dimensionful["error"]["code"], "UnsupportedExpressionError");
        let excessive = run(request(
            binary(
                "^",
                binary("^", number("9"), number("4096")),
                number("4096"),
            ),
            json!({}),
        ));
        assert_eq!(excessive["error"]["code"], "ResourceLimitError");
        let missing = run(request(variable("x"), json!({})));
        assert_eq!(missing["error"]["code"], "MissingVariableError");
        let zero = run(request(binary("/", number("1"), number("0")), json!({})));
        assert_eq!(zero["error"]["code"], "DivisionByZeroError");
        assert_eq!(zero["error"]["class"], "indeterminate");
        let call = run(request(
            json!({"type":"call","name":"sqrt","argument":number("2")}),
            json!({}),
        ));
        assert_eq!(call["error"]["code"], "UnsupportedExpressionError");
    }

    #[test]
    fn parser_and_register_refusals_remain_distinct() {
        let bad_syntax = run(request(
            variable("x"),
            json!({"x":{"value":"1","unit":"m^^2"}}),
        ));
        assert_eq!(bad_syntax["error"]["code"], "UnitSyntaxError");
        assert!(bad_syntax["error"]["parser"]["byte_offset"].is_u64());
        let unknown_conversion = run(request(
            variable("x"),
            json!({"x":{"value":"1","unit":"deg"}}),
        ));
        assert_eq!(
            unknown_conversion["error"]["code"],
            "MissingUnitConversionError"
        );
        let inconsistent = run(request(
            variable("x"),
            json!({"x":{"value":"1","unit":"C"}}),
        ));
        assert_eq!(
            inconsistent["error"]["code"],
            "InconsistentUnitDimensionsError"
        );
        let scalar = run(request(number("2.5"), json!({})));
        assert_eq!(scalar["result"]["value"]["magnitude"], "5/2");
        assert_eq!(scalar["result"]["value"]["dimensions"], json!({}));
        assert_eq!(scalar["result"]["value"]["terms"], json!([]));
    }
}

#[cfg(not(feature = "quantity"))]
#[test]
fn numeric_only_build_refuses_quantity_profile() {
    let result = run(json!({"schema":PROFILE,"ast":{"type":"number","text":"1"}}));
    assert_eq!(result["error"]["code"], "UnavailableProfileError");
}

#[cfg(feature = "quantity")]
#[test]
fn source_caller_register_wires() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/quantity-register.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 32);
    // Separate source profiles record changes from UnitsDB data and corrected
    // quantity scale semantics. Each pins the superseded expectation byte for
    // byte; the legacy UnitsDB 2.2.0 (c2c7d18) fixture stays frozen.
    let profile: Value = serde_json::from_str(include_str!(
        "../fixtures/source-profile-unitsdb-b210137.json"
    ))
    .unwrap();
    let scale_profile: Value = serde_json::from_str(include_str!(
        "../fixtures/source-profile-unitsml-cdfbcc3.json"
    ))
    .unwrap();
    let superseded: std::collections::BTreeMap<&str, &Value> = profile["superseded_legacy_rows"]
        .as_array()
        .unwrap()
        .iter()
        .chain(scale_profile["superseded_legacy_rows"].as_array().unwrap())
        .map(|row| (row["id"].as_str().unwrap(), row))
        .collect();
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let expected = match superseded.get(id) {
            Some(row) => {
                assert_eq!(row["legacy_expected_wire"], case["expected_wire"], "{id}");
                assert_eq!(row["request_json"], case["request_json"], "{id}");
                row["expected_wire"].as_str().unwrap()
            }
            None => case["expected_wire"].as_str().unwrap(),
        };
        assert_eq!(
            plurimath_numeric::evaluate_json(case["request_json"].as_str().unwrap()),
            expected,
            "{id}"
        );
    }
    assert_eq!(superseded.len(), 3);
}

#[cfg(feature = "quantity")]
#[test]
fn source_profile_unitsdb_b210137_definitions() {
    let profile: Value = serde_json::from_str(include_str!(
        "../fixtures/source-profile-unitsdb-b210137.json"
    ))
    .unwrap();
    let probes = profile["definition_probes"].as_array().unwrap();
    assert_eq!(probes.len(), 17);
    for probe in probes {
        assert_eq!(
            plurimath_numeric::evaluate_json(probe["request_json"].as_str().unwrap()),
            probe["expected_wire"].as_str().unwrap(),
            "{}",
            probe["id"]
        );
    }
    // A pound registration applies to the pound itself but does not define
    // cwt (US) through its root_units chain (unitsml-ruby behaviour).
    let by_id = |id: &str| probes.iter().find(|p| p["id"] == id).unwrap();
    let wire = |id: &str| -> Value {
        serde_json::from_str(by_id(id)["expected_wire"].as_str().unwrap()).unwrap()
    };
    assert_eq!(
        wire("def-lb-to-kg-lb-register")["result"]["converted"]["magnitude"],
        "45359237/100000000"
    );
    for id in [
        "def-cwt-us-to-lb-lb-register",
        "def-cwt-us-to-kg-lb-register",
    ] {
        assert_eq!(
            wire(id)["error"]["code"],
            "MissingUnitConversionError",
            "{id}"
        );
    }
    // A cwt (UK) registration no longer leaks into cwt (US).
    assert_eq!(wire("def-cwt-us-uk-register-no-leak")["ok"], false);
    // Gram-force from data alone: 1 gf = 0.00980665 N, 1 kp = 1 kgf = 1000 gf = 9.80665 N.
    let converted = |id: &str| wire(id)["result"]["converted"]["magnitude"].clone();
    assert_eq!(converted("def-gf-N-no-register"), "196133/20000000");
    assert_eq!(converted("def-kp-N-no-register"), "196133/20000");
    assert_eq!(converted("def-kgf-N-no-register"), "196133/20000");
    assert_eq!(converted("def-kp-gf-no-register"), "1000");
    assert_eq!(converted("def-kp-kgf-no-register"), "1");
    // Every unit resolved by symbol: only the data-changed units differ from 2.2.0.
    for change in profile["unit_resolution_changes"].as_array().unwrap() {
        assert_eq!(
            plurimath_numeric::evaluate_json(change["request_json"].as_str().unwrap()),
            change["expected_wire"].as_str().unwrap(),
            "{}",
            change["id"]
        );
    }
    assert_eq!(
        profile["unit_resolution_changes"].as_array().unwrap().len(),
        11
    );
}
