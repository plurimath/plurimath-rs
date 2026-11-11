use super::parse;
use serde_json::{json, Value};

#[test]
fn source_xml_models_match_common_ir() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../fixtures/xml-parse/source-models.json")).unwrap();
    for case in fixtures["cases"].as_array().unwrap() {
        let actual = parse(
            case["input"].as_str().unwrap(),
            case["input_format"].as_str().unwrap(),
            &json!({}),
        );
        assert_eq!(actual, Ok(case["model"].clone()), "{}", case["id"]);
    }
}

#[test]
fn xml_errors_and_unimplemented_nodes_are_explicit() {
    let malformed = parse("<math><mi>x</math>", "mathml", &json!({})).unwrap_err();
    assert_eq!(malformed["code"], "PARSE_ERROR");
    assert_eq!(malformed["byte_offset"], 0);
    let incomplete = parse(
        "<math><maction><mi>x</mi></maction></math>",
        "mathml",
        &json!({}),
    )
    .unwrap_err();
    assert_eq!(incomplete["code"], "INCOMPLETE_XML_TRANSLATION");
}

#[test]
fn html_requires_its_source_markup_grammar() {
    let input = "<html><body><math><mi>x</mi></math></body></html>";
    let error = parse(input, "html", &json!({})).unwrap_err();
    assert_eq!(error["code"], "INCOMPLETE_HTML_GRAMMAR");
}

#[test]
fn namespace_and_empty_argument_positions_follow_source_reader() {
    let model = parse(
        "<math xmlns:q=\"urn:foreign\"><mi q:mathvariant=\"bold\">x</mi></math>",
        "mathml",
        &json!({}),
    )
    .unwrap();
    assert_eq!(
        model["fields"]["value"][0]["class"],
        "Math::Symbols::Symbol"
    );
    let model = parse(
        "<math><mfrac><none/><mi>x</mi></mfrac></math>",
        "mathml",
        &json!({}),
    )
    .unwrap();
    assert_eq!(
        model["fields"]["value"][0]["fields"]["parameter_one"],
        Value::Null
    );
    assert_eq!(
        model["fields"]["value"][0]["fields"]["parameter_two"]["fields"]["value"],
        "x"
    );
}

#[test]
fn source_text_setter_decodes_its_literal_entity_once() {
    let model = parse(
        "<math><mtext>a&amp;amp;b</mtext></math>",
        "mathml",
        &json!({}),
    )
    .unwrap();
    assert_eq!(
        model["fields"]["value"][0]["fields"]["parameter_one"],
        "aunicode[:&]b"
    );
}
