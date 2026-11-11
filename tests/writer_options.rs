use plurimath_facade::writer;
use serde_json::Value;

fn check_source_rows(input: &str, expected_count: usize) {
    let fixture: Value = serde_json::from_str(input).unwrap();
    assert_eq!(
        fixture["source"],
        "0a1df68a5903d43da13b1e1ef85b6c0523feb1b4"
    );
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), expected_count);
    for row in rows {
        let format = row["format"].as_str().unwrap();
        let actual = writer::write(&row["model"], format, &row["options"]);
        if let Some(expected) = row["expected"]["output"].as_str() {
            assert_eq!(
                actual.unwrap_or_else(|error| panic!("{}: {error}", row["id"])),
                expected,
                "{}",
                row["id"]
            );
        } else {
            let error = actual.expect_err(&format!("source exception {}", row["id"]));
            let prefix = match row["expected"]["error"]["class"].as_str().unwrap() {
                "NoMethodError" => "source_no_method_error: ",
                "Plurimath::Math::ParseError" => "source_render_error: ",
                class => panic!("unclassified source exception {class}: {}", row["id"]),
            };
            assert!(
                error.starts_with(prefix),
                "{}: expected {prefix}, got {error}",
                row["id"]
            );
        }
    }
}

#[test]
fn nested_accents_spacing_and_intent_match_source() {
    check_source_rows(
        include_str!("../fixtures/writer-options/accents-and-spacing.json"),
        274,
    );
}

#[test]
fn linebreak_continuations_and_array_aliases_match_source() {
    check_source_rows(
        include_str!("../fixtures/writer-options/linebreak-state.json"),
        336,
    );
}

#[test]
fn nullable_fences_multi_node_continuations_and_color_match_source() {
    check_source_rows(
        include_str!("../fixtures/writer-options/supplemental.json"),
        136,
    );
}
