use plurimath_facade::writer;
use serde_json::{json, Value};
#[test]
fn source_table_xml_shapes_and_refusals() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/table-xml-source.json")).unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        for (format, expected) in row["outputs"].as_object().unwrap() {
            let actual = writer::write(&row["model"], format, &json!({}));
            if let Some(expected) = expected.as_str() {
                assert_eq!(actual.unwrap(), expected, "{} {}", row["id"], format);
            } else {
                assert!(actual.is_err(), "source refusal {} {}", row["id"], format);
            }
        }
    }
}
