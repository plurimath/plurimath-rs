use plurimath_facade::writer;
use serde_json::Value;
fn check(source: &str, total: usize, successes: usize, refusals: usize) {
    let source: Value = serde_json::from_str(source).unwrap();
    let (mut seen, mut exact, mut rejected) = (0, 0, 0);
    for row in source["rows"].as_array().unwrap() {
        for (format, expected) in row["writers"].as_object().unwrap() {
            seen += 1;
            let actual = writer::write(&row["model"], format, &row["options"]);
            if let Some(expected) = expected["value"].as_str() {
                assert_eq!(
                    actual.unwrap_or_else(|error| panic!("{} {format}: {error}", row["id"])),
                    expected,
                    "{} {format}",
                    row["id"]
                );
                exact += 1;
            } else {
                assert!(actual.is_err(), "source refusal {} {format}", row["id"]);
                rejected += 1;
            }
        }
    }
    assert_eq!((seen, exact, rejected), (total, successes, refusals));
}
#[test]
fn source_mod_log_fields_scripts_hiding_options_and_continuations() {
    check(
        include_str!("../fixtures/mod-log/modlog-source.json"),
        6966,
        4396,
        2570,
    );
}
#[test]
fn shared_symbol_unicode_priority_and_mini_values() {
    check(
        include_str!("../fixtures/mod-log/symbols-source.json"),
        330,
        327,
        3,
    );
}
