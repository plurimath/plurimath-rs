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
    }
    let reused: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&request.to_string())).unwrap();
    assert_eq!(reused["ok"], true);
}
