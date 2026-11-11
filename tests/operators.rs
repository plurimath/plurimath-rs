use plurimath_facade::writer;
use serde_json::Value;

#[test]
fn source_operator_outputs_and_refusals() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/operator-writers/source-operators.json"
    ))
    .unwrap();
    let gaps: Value = serde_json::from_str(include_str!(
        "../fixtures/operator-writers/explicit-gaps.json"
    ))
    .unwrap();
    let mut exact = 0;
    let mut refusals = 0;
    let mut total = 0;
    for row in fixture["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            total += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(value) = expected["value"].as_str() {
                match actual {
                    Ok(actual) => {
                        assert_eq!(actual, value, "{} {format}", row["id"]);
                        exact += 1;
                    }
                    Err(error) => {
                        let gap = gaps["gaps"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|gap| gap["id"] == row["id"] && gap["format"] == format.as_str())
                            .unwrap_or_else(|| {
                                panic!("unexpected refusal {} {format}: {error}", row["id"])
                            });
                        assert_eq!(
                            error,
                            gap["reason"].as_str().unwrap(),
                            "{} {format}",
                            row["id"]
                        );
                    }
                }
            } else {
                assert!(actual.is_err(), "source refusal {} {format}", row["id"]);
                refusals += 1;
            }
        }
    }
    assert_eq!(total, 677);
    assert_eq!(refusals, 128);
    assert!(exact >= 401, "qualified source behavior regressed: {exact}");
}
