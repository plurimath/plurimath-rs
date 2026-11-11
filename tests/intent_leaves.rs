use plurimath_facade::writer;
use serde_json::Value;

fn check(name: &str, source: &str, expected: (usize, usize, usize, usize)) {
    let source: Value = serde_json::from_str(source).unwrap();
    let gaps: Value =
        serde_json::from_str(include_str!("../fixtures/intent-leaves/explicit-gaps.json")).unwrap();
    let (mut total, mut exact, mut refused, mut unsupported) = (0, 0, 0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, output) in row["writers"].as_object().unwrap() {
            total += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(value) = output["value"].as_str() {
                match actual {
                    Ok(actual) => {
                        assert_eq!(actual, value, "{name} {} {format}", row["id"]);
                        exact += 1;
                    }
                    Err(error) => {
                        let gap = gaps["gaps"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|gap| {
                                gap["fixture"] == name
                                    && gap["id"] == row["id"]
                                    && gap["format"] == format.as_str()
                            })
                            .unwrap_or_else(|| {
                                panic!("unexpected {name} {} {format}: {error}", row["id"])
                            });
                        assert_eq!(error, gap["reason"].as_str().unwrap());
                        unsupported += 1;
                    }
                }
            } else {
                assert!(
                    actual.is_err(),
                    "source refusal {name} {} {format}",
                    row["id"]
                );
                refused += 1;
            }
        }
    }
    assert_eq!((total, exact, refused, unsupported), expected);
}
#[test]
fn explicit_intent_states() {
    check(
        "intent",
        include_str!("../fixtures/intent-leaves/intent-source.json"),
        (954, 456, 491, 7),
    );
}
#[test]
fn explicit_intent_continuations() {
    check(
        "intent-extra",
        include_str!("../fixtures/intent-leaves/intent-extra-source.json"),
        (84, 13, 67, 4),
    );
}
#[test]
fn sup_root_ms_fields_setters_context_and_continuations() {
    check(
        "leaves",
        include_str!("../fixtures/intent-leaves/leaves-source.json"),
        (2772, 739, 1981, 52),
    );
}
#[test]
fn existing_source_model_reference() {
    check(
        "reference",
        include_str!("../fixtures/intent-leaves/reference-source.json"),
        (74, 70, 4, 0),
    );
}

#[test]
fn source_arg_annotations_and_inherited_binary_procedures() {
    check(
        "arg",
        include_str!("../fixtures/intent-leaves/arg-source.json"),
        (954, 442, 505, 7),
    );
}
