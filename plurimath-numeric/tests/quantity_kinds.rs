//! F40 quantity kinds and primary quantities against the independent Ruby
//! oracle generation `ruby-unitsml-features/v1` (unitsml-ruby 2337a12,
//! testsuite f40/evaluator-reference-v2-claude d5d0da8), copied byte-for-byte
//! to fixtures/f40-ruby-unitsml-features-v1.json.
#![cfg(feature = "quantity")]

use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};
use unitsml_rs::QuantityKind;

const PROFILE: &str = "plurimath.evaluate.quantity/1";

fn cases(family: &str) -> Vec<Value> {
    let all: Value = serde_json::from_str(include_str!(
        "../fixtures/f40-ruby-unitsml-features-v1.json"
    ))
    .unwrap();
    all["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["family"] == family)
        .cloned()
        .collect()
}

fn run(request: Value) -> Value {
    serde_json::from_str(&evaluate_json(&request.to_string())).unwrap()
}

fn operand(side: &Value) -> Value {
    let value = match &side["value"] {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap().to_owned(),
    };
    let mut binding = json!({"value": value, "unit": side["unit"]});
    if let Some(kind) = side["kind"].as_str() {
        binding["kind"] = json!(kind);
    }
    binding
}

/// Ruby encodes a rational result as an integer or {"rational":[n,d]};
/// the native magnitude is the reduced rational string.
fn magnitude(expected: &Value) -> String {
    match expected {
        Value::Number(n) => n.to_string(),
        Value::Object(o) => {
            let pair = o["rational"].as_array().unwrap();
            let (n, d) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
            if d == "1" {
                n.to_owned()
            } else {
                format!("{n}/{d}")
            }
        }
        other => panic!("unexpected value {other}"),
    }
}

fn error_class(expected: &Value) -> &str {
    expected["error_class"]
        .as_str()
        .unwrap()
        .rsplit("::")
        .next()
        .unwrap()
}

#[test]
fn quantity_kinds_match_ruby_oracle() {
    let rows = cases("quantity_kinds");
    assert_eq!(rows.len(), 124);
    let ast = json!({"type":"binary","operator":"+",
        "left":{"type":"variable","name":"x"},"right":{"type":"variable","name":"y"}});
    let mut executed = 0;
    for case in &rows {
        let (input, expected) = (&case["input"], &case["expected"]);
        let response = if input.get("left").is_some() {
            let bindings = json!({"x": operand(&input["left"]), "y": operand(&input["right"])});
            run(json!({"schema":PROFILE,"ast":ast,"bindings":bindings}))
        } else {
            // Construction-only rows: the unit and kind are on the expectation.
            let x = json!({"value":"1","unit":expected["unit"],"kind":expected["kind"]});
            run(json!({"schema":PROFILE,"ast":{"type":"variable","name":"x"},"bindings":{"x":x}}))
        };
        if expected["outcome"] == "ok" {
            assert_eq!(response["ok"], true, "{}: {response}", case["id"]);
            let value = &response["result"]["value"];
            assert_eq!(
                value["magnitude"],
                json!(magnitude(&expected["value"])),
                "{}",
                case["id"]
            );
            assert_eq!(value["kind"], expected["kind"], "{}", case["id"]);
            let left = run(
                json!({"schema":PROFILE,"ast":{"type":"variable","name":"x"},
                "bindings":{"x":{"value":"1","unit":expected["unit"]}}}),
            );
            assert_eq!(
                value["terms"], left["result"]["value"]["terms"],
                "{}",
                case["id"]
            );
        } else {
            assert_eq!(response["ok"], false, "{}: {response}", case["id"]);
            assert_eq!(
                response["error"]["code"],
                json!(error_class(expected)),
                "{}: {response}",
                case["id"]
            );
        }
        executed += 1;
    }
    assert_eq!(executed, 124);
}

#[test]
fn untyped_requests_carry_no_kind_field() {
    let response = run(
        json!({"schema":PROFILE,"ast":{"type":"variable","name":"x"},
        "bindings":{"x":{"value":"2","unit":"J"}}}),
    );
    assert!(
        response["result"]["value"].get("kind").is_none(),
        "{response}"
    );
}

#[test]
fn kinds_drop_through_products_and_survive_negation_and_conversion() {
    let x = json!({"value":"2","unit":"J","kind":"NISTq17"});
    let product = run(json!({"schema":PROFILE,
        "ast":{"type":"binary","operator":"*","left":{"type":"variable","name":"x"},"right":{"type":"variable","name":"x"}},
        "bindings":{"x":x}}));
    assert!(
        product["result"]["value"].get("kind").is_none(),
        "{product}"
    );
    let negated = run(json!({"schema":PROFILE,
        "ast":{"type":"unary","operator":"-","operand":{"type":"variable","name":"x"}},
        "bindings":{"x":x},"convert_to":"kJ"}));
    assert_eq!(negated["result"]["value"]["kind"], "NISTq17", "{negated}");
    assert_eq!(
        negated["result"]["converted"]["kind"], "NISTq17",
        "{negated}"
    );
}

#[test]
fn primary_quantities_match_ruby_oracle_in_every_order() {
    let rows = cases("primary_quantity");
    assert_eq!(rows.len(), 95);
    for case in &rows {
        let (input, expected) = (&case["input"], &case["expected"]);
        let unit = expected["unit"].as_str().unwrap();
        let refs = QuantityKind::unit_references(unit).expect(unit);
        let ids: Vec<_> = refs.iter().map(|r| r["id"].clone()).collect();
        assert_eq!(
            Value::Array(ids),
            input["references"],
            "{unit}: data references"
        );
        let mut reversed = refs.to_vec();
        reversed.reverse();
        let mut rotated = refs.to_vec();
        rotated.rotate_left(1);
        for order in [refs.to_vec(), reversed, rotated] {
            let primary = QuantityKind::primary_among(unit, &order).unwrap().unwrap();
            assert_eq!(primary.id(), expected["primary"], "{unit}");
        }
        let primary = QuantityKind::primary_for_unit(unit).unwrap().unwrap();
        assert_eq!(primary.id(), expected["primary"], "{unit}");
        assert_eq!(
            refs[0]["id"] == expected["primary"],
            expected["first_reference_is_primary"] == true,
            "{unit}"
        );
    }
}

#[test]
fn primary_selection_refuses_ambiguous_data() {
    let none = [json!({"id":"NISTq17"}), json!({"id":"NISTq1"})];
    let many = [
        json!({"id":"NISTq17","primary":true}),
        json!({"id":"NISTq1","primary":true}),
    ];
    for refs in [&none[..], &many[..]] {
        let error = QuantityKind::primary_among("u", refs).unwrap_err();
        assert_eq!(error.code(), "PrimaryQuantityError");
    }
    let single = [json!({"id":"NISTq17"})];
    assert_eq!(
        QuantityKind::primary_among("u", &single)
            .unwrap()
            .unwrap()
            .id(),
        "NISTq17"
    );
    assert!(QuantityKind::primary_among("u", &[]).unwrap().is_none());
}

#[test]
fn binding_kind_primary_uses_the_flag() {
    let response = run(
        json!({"schema":PROFILE,"ast":{"type":"variable","name":"x"},
        "bindings":{"x":{"value":"1","unit":"J","kind":"primary"}}}),
    );
    let expected = QuantityKind::primary_for_unit("NISTu13")
        .unwrap()
        .map(|k| k.id());
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(
        response["result"]["value"]["kind"].as_str(),
        expected,
        "{response}"
    );
}
