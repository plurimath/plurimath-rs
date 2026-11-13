use plurimath_facade::writer;
use serde_json::Value;
fn assert_reference(source: &str, outcomes: usize) {
    let data: Value = serde_json::from_str(source).unwrap();
    let mut count = 0;
    for row in data["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            count += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(value) = expected["value"].as_str() {
                assert_eq!(actual.unwrap(), value, "{} {format}", row["id"]);
            } else {
                assert!(actual.is_err(), "source refusal {} {format}", row["id"]);
            }
        }
    }
    assert_eq!(count, outcomes);
}
#[test]
fn source_family_intent_placeholders_and_linebreak_state() {
    assert_reference(
        include_str!("../fixtures/nary-family/family-source.json"),
        2136,
    );
}
#[test]
fn original_twenty_four_wrong_linebreak_successes_are_source_exact() {
    assert_reference(
        include_str!("../fixtures/nary-family/linebreak-regressions.json"),
        24,
    );
}
