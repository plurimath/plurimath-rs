use plurimath_facade::writer;
use serde_json::Value;

fn source_cases(source: &str, total: usize, successes: usize, refusals: usize) {
    let source: Value = serde_json::from_str(source).unwrap();
    let gaps: Value =
        serde_json::from_str(include_str!("../fixtures/layout-nodes/explicit-gaps.json")).unwrap();
    let (mut seen, mut exact, mut rejected) = (0, 0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            seen += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(expected) = expected["value"].as_str() {
                match actual {
                    Ok(actual) => {
                        assert_eq!(actual, expected, "{} {format}", row["id"]);
                        exact += 1;
                    }
                    Err(error) => {
                        let gap = gaps["gaps"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|gap| gap["id"] == row["id"] && gap["format"] == format.as_str())
                            .unwrap_or_else(|| panic!("{} {format}: {error}", row["id"]));
                        assert_eq!(error, gap["reason"].as_str().unwrap());
                    }
                }
            } else {
                assert!(actual.is_err(), "source refusal {} {format}", row["id"]);
                rejected += 1;
            }
        }
    }
    assert_eq!((seen, exact, rejected), (total, successes, refusals));
}

#[test]
fn source_layout_fields_options_setters_suppression_and_continuations() {
    source_cases(
        include_str!("../fixtures/layout-nodes/layout-source.json"),
        2574,
        1732,
        842,
    );
}

#[test]
fn existing_layout_reference() {
    source_cases(
        include_str!("../fixtures/layout-nodes/reference-source.json"),
        156,
        133,
        19,
    );
}
