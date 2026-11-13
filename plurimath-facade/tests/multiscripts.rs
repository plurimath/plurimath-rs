use plurimath_facade::writer;
use serde_json::Value;

fn assert_source(fixture: &str, source: &str, expected: (usize, usize, usize)) {
    let data: Value = serde_json::from_str(source).unwrap();
    let gaps: Value = serde_json::from_str(include_str!(
        "../fixtures/multiscript-writers/explicit-gaps.json"
    ))
    .unwrap();
    let (mut total, mut exact, mut refusals) = (0, 0, 0);
    for row in data["rows"].as_array().unwrap() {
        for (format, output) in row["writers"].as_object().unwrap() {
            total += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(value) = output["value"].as_str() {
                match actual {
                    Ok(actual) => {
                        assert_eq!(actual, value, "{fixture} {} {format}", row["id"]);
                        exact += 1;
                    }
                    Err(error) => {
                        let gap = gaps["gaps"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|g| {
                                g["fixture"] == fixture
                                    && g["id"] == row["id"]
                                    && g["format"] == format.as_str()
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "unexpected refusal {fixture} {} {format}: {error}",
                                    row["id"]
                                )
                            });
                        assert_eq!(
                            error,
                            gap["reason"].as_str().unwrap(),
                            "{fixture} {} {format}",
                            row["id"]
                        );
                    }
                }
            } else {
                assert!(
                    actual.is_err(),
                    "source refusal {fixture} {} {format}",
                    row["id"]
                );
                refusals += 1;
            }
        }
    }
    assert_eq!(total, expected.0);
    assert!(exact >= expected.1);
    assert_eq!(refusals, expected.2);
}
#[test]
fn constructor_setter_and_uneven_prescript_states() {
    assert_source(
        "multiscript",
        include_str!("../fixtures/multiscript-writers/multiscript-source.json"),
        (2118, 489, 1542),
    );
}
#[test]
fn plain_linebreak_payload_and_attribute_states() {
    assert_source(
        "linebreak",
        include_str!("../fixtures/multiscript-writers/linebreak-source.json"),
        (588, 385, 203),
    );
}
#[test]
fn existing_multiscript_source_reference() {
    assert_source(
        "reference",
        include_str!("../fixtures/multiscript-writers/reference-source.json"),
        (92, 64, 20),
    );
}
