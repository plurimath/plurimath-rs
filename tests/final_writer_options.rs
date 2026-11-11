use plurimath_facade::writer;
use serde_json::Value;

fn source_cases(
    name: &str,
    source: &str,
    total: usize,
    minimum_exact: usize,
    source_refusals: usize,
) {
    let source: Value = serde_json::from_str(source).unwrap();
    let gaps: Value = serde_json::from_str(include_str!(
        "../fixtures/final-writer-options/explicit-gaps.json"
    ))
    .unwrap();
    let (mut seen, mut exact, mut refused) = (0, 0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            seen += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(expected) = expected["value"].as_str() {
                match actual {
                    Ok(actual) => {
                        assert_eq!(actual, expected, "{name} {} {format}", row["id"]);
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
                        assert_eq!(
                            error,
                            gap["reason"].as_str().unwrap(),
                            "{name} {} {format}",
                            row["id"]
                        );
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
    assert_eq!(seen, total);
    assert!(
        exact >= minimum_exact,
        "new unsupported outcomes: {exact} < {minimum_exact}"
    );
    assert_eq!(refused, source_refusals);
}
#[test]
fn source_limit_under_over_and_fraction_option_procedures() {
    source_cases(
        "final",
        include_str!("../fixtures/final-writer-options/final-source.json"),
        8364,
        6541,
        1747,
    );
}
#[test]
fn source_literal_and_encoded_prime_callers() {
    source_cases(
        "prime",
        include_str!("../fixtures/final-writer-options/prime-source.json"),
        180,
        178,
        2,
    );
}
#[test]
fn unchanged_affected_model_goldens() {
    source_cases(
        "affected",
        include_str!("../fixtures/final-writer-options/affected-source.json"),
        126,
        120,
        0,
    );
}

#[test]
fn source_display_callers_and_actual_unicode_fraction_parse() {
    source_cases(
        "callers",
        include_str!("../fixtures/final-writer-options/callers-source.json"),
        216,
        182,
        34,
    );
}
