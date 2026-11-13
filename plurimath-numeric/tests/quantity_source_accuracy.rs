#![cfg(feature = "quantity")]
use num_bigint::BigInt;
use num_rational::BigRational;
use serde_json::Value;

fn source_magnitude(value: &Value) -> BigRational {
    if let Some(integer) = value.as_i64() {
        return BigRational::from_integer(integer.into());
    }
    if let Some(pair) = value["rational"].as_array() {
        return BigRational::new(
            pair[0].as_str().unwrap().parse().unwrap(),
            pair[1].as_str().unwrap().parse().unwrap(),
        );
    }
    if let Some(bits) = value["binary64"].as_str() {
        return BigRational::from_float(f64::from_bits(u64::from_str_radix(bits, 16).unwrap()))
            .unwrap();
    }
    let decimal = value["decimal"].as_str().unwrap();
    let places = decimal.split('.').nth(1).map_or(0, str::len);
    BigRational::new(
        decimal.replace('.', "").parse::<BigInt>().unwrap(),
        BigInt::from(10).pow(places.try_into().unwrap()),
    )
}

#[test]
fn ruby_numeric_classes_survive_quantity_conversion_and_unused_bindings() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/quantity-source-accuracy-v1.json")).unwrap();
    let rows = corpus["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 13);
    for row in rows {
        let wire = plurimath_numeric::evaluate_json(&row["request"].to_string());
        let actual: Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(actual["ok"], true, "{}: {}", row["id"], wire);
        assert_eq!(
            actual["result"]["accuracy"], row["source"]["accuracy"],
            "{}: {}",
            row["id"], wire
        );
        assert_eq!(
            actual["result"]["value"]["magnitude"],
            source_magnitude(&row["source"]["value"]).to_string(),
            "{}: {}",
            row["id"],
            wire
        );
    }
}

#[test]
fn exact_strings_remain_exact_and_invalid_numeric_tags_refuse() {
    let request = serde_json::json!({"schema":"plurimath.evaluate.quantity/1",
        "ast":{"type":"variable","name":"x"},"bindings":{"x":{"value":"2.50","unit":"m"}}});
    let actual: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&request.to_string())).unwrap();
    assert_eq!(actual["result"]["accuracy"]["symbol"], "exact");
    assert!(actual["result"]["value"].get("numeric_value").is_none());
    for value in [
        serde_json::json!({"decimal":"NaN"}),
        serde_json::json!({"rational":["1","0"]}),
        serde_json::json!({"binary64":"7ff0000000000000"}),
        serde_json::json!({"binary64":"not-bits"}),
    ] {
        let mut invalid = request.clone();
        invalid["bindings"]["x"]["value"] = value;
        let actual: Value =
            serde_json::from_str(&plurimath_numeric::evaluate_json(&invalid.to_string())).unwrap();
        assert_eq!(actual["ok"], false, "{}", actual);
        assert_eq!(actual["error"]["code"], "InvalidBindingError", "{}", actual);
    }
    let reused: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&request.to_string())).unwrap();
    assert_eq!(reused["ok"], true);
}

#[test]
fn frozen_typed_generation_preserves_exact_source_numeric_wire() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/quantity-accuracy-typed-v1.json")).unwrap();
    let numeric: Value = serde_json::from_str(include_str!(
        "../fixtures/quantity-accuracy-numeric-value-v1.json"
    ))
    .unwrap();
    let units: Value = serde_json::from_str(include_str!(
        "../fixtures/quantity-source-unit-maps-v1.json"
    ))
    .unwrap();
    let rows = corpus["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 36);
    for row in rows {
        let request: Value = serde_json::from_str(row["request_json"].as_str().unwrap()).unwrap();
        let wire = plurimath_numeric::evaluate_json(row["request_json"].as_str().unwrap());
        let actual: Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(actual["ok"], true, "{}: {}", row["id"], wire);
        assert_eq!(
            actual["result"]["accuracy"], row["expected"]["accuracy"],
            "{}: {}",
            row["id"], wire
        );
        assert_eq!(
            actual["result"]["value"]["magnitude"],
            source_magnitude(&row["expected"]["value"]).to_string(),
            "{}: {}",
            row["id"],
            wire
        );
        let unit = units["units"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["unit"] == row["expected"]["unit"])
            .unwrap();
        assert_eq!(
            actual["result"]["value"]["terms"], unit["terms"],
            "{}: {}",
            row["id"], wire
        );
        assert_eq!(
            actual["result"]["value"]["dimensions"], unit["dimensions"],
            "{}: {}",
            row["id"], wire
        );
        let factor: BigRational = unit["factor"].as_str().unwrap().parse().unwrap();
        let offset: BigRational = unit["offset"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            actual["result"]["value"]["canonical_magnitude"],
            (source_magnitude(&row["expected"]["value"]) * factor + offset).to_string(),
            "{}: {}",
            row["id"],
            wire
        );
        let tagged = request["bindings"].as_object().unwrap().values().any(|v| {
            let value = v.get("value").unwrap_or(v);
            ["decimal", "rational", "binary64"]
                .iter()
                .any(|key| value.get(*key).is_some())
        });
        if tagged {
            let expected = numeric["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == row["id"])
                .unwrap();
            assert_eq!(
                actual["result"]["value"]["numeric_value"], expected["numeric_value"],
                "{}: {}",
                row["id"], wire
            );
        } else {
            assert_eq!(row["expected"]["value_class"], "Rational");
            assert!(actual["result"]["value"].get("numeric_value").is_none());
        }
    }
}

#[test]
fn explicit_conversion_rationalizes_typed_magnitude_and_signed_zero_survives() {
    let request = serde_json::json!({"schema":"plurimath.evaluate.quantity/1",
        "ast":{"type":"variable","name":"x"},"bindings":{"x":{"value":{"decimal":"2.5"},"unit":"m"}},"convert_to":"cm"});
    let actual: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&request.to_string())).unwrap();
    assert_eq!(
        actual["result"]["value"]["numeric_value"],
        serde_json::json!({"kind":"decimal","value":"0.25e1","precision":2})
    );
    assert_eq!(
        actual["result"]["converted"]["numeric_value"],
        serde_json::json!({"kind":"rational","numerator":"250","denominator":"1"})
    );
    assert_eq!(actual["result"]["accuracy"]["symbol"], "limited_2");
    let mut zero = request;
    zero["bindings"]["x"]["value"] = serde_json::json!({"binary64":"8000000000000000"});
    let actual: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&zero.to_string())).unwrap();
    assert_eq!(
        actual["result"]["value"]["numeric_value"],
        serde_json::json!({"kind":"float","bits":"8000000000000000"})
    );
    assert_eq!(
        actual["result"]["converted"]["numeric_value"],
        serde_json::json!({"kind":"rational","numerator":"0","denominator":"1"})
    );
}

#[test]
fn quantity_float_overflow_preserves_the_source_refusal_class() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../fixtures/quantity-source-float-refusals-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 3);
    for row in corpus["cases"].as_array().unwrap() {
        let actual: Value = serde_json::from_str(&plurimath_numeric::evaluate_json(
            &row["request"].to_string(),
        ))
        .unwrap();
        assert_eq!(actual["ok"], false, "{}: {}", row["id"], actual);
        assert_eq!(
            actual["error"]["code"], row["expected_code"],
            "{}: {}",
            row["id"], actual
        );
    }
}
