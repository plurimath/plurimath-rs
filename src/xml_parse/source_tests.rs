use super::{parse, source_document_rows};
use moxml::document::{ContentRow, ElementRow};
use serde_json::{json, Map, Value};

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

#[test]
fn frozen_source_xml_models_remain_exact() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../fixtures/xml-parse/recovery-frozen-models.json"
    ))
    .unwrap();
    assert_eq!(fixture["rows"].as_array().unwrap().len(), 477);
    for row in fixture["rows"].as_array().unwrap() {
        assert_eq!(
            parse(
                row["input"].as_str().unwrap(),
                row["format"].as_str().unwrap(),
                &json!({})
            )
            .unwrap(),
            row["model"],
            "source model {}",
            row["id"]
        );
    }
}

fn recovery_dom(rows: &[ElementRow], index: usize) -> Value {
    let row = &rows[index];
    let mut children = Vec::new();
    let mut text = String::new();
    for content in &row.content {
        let child = match content {
            ContentRow::Element(index) => {
                let child = recovery_dom(rows, *index);
                text.push_str(child["text"].as_str().unwrap());
                child
            }
            ContentRow::Text(value) => {
                text.push_str(value);
                json!({"kind":"text","text":value})
            }
            ContentRow::CData(value) => {
                text.push_str(value);
                json!({"kind":"cdata","text":value})
            }
            ContentRow::Comment(value) => json!({"kind":"comment","text":value}),
            ContentRow::ProcessingInstruction { target, data } => {
                json!({"kind":"pi","target":target,"text":data})
            }
        };
        children.push(child);
    }
    let name = |name: &moxml::document::QName| {
        name.prefix
            .as_ref()
            .map(|prefix| format!("{prefix}:{}", name.local))
            .unwrap_or_else(|| name.local.clone())
    };
    let attributes: Map<String, Value> = row
        .attributes
        .iter()
        .map(|attribute| (name(&attribute.name), json!(attribute.value)))
        .collect();
    json!({"kind":"element","name":name(&row.name),"text":text,"attributes":attributes,"children":children})
}

#[test]
fn source_lexical_values_refusals_and_retained_policy_gates() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../fixtures/xml-parse/recovery-lexical-cases.json"
    ))
    .unwrap();
    let mut limits = 0;
    for case in fixture["rows"].as_array().unwrap() {
        let actual = source_document_rows(case["xml"].as_str().unwrap());
        if let Some(limit) = case.get("native_policy_limit") {
            let error = actual.unwrap_err();
            assert!(
                error["message"]
                    .as_str()
                    .unwrap()
                    .contains(limit["message"].as_str().unwrap()),
                "policy {}: {error}",
                case["id"]
            );
            limits += 1;
        } else if case.get("error").is_some() {
            assert!(actual.is_err(), "source refusal {}", case["id"]);
        } else {
            assert_eq!(
                recovery_dom(&actual.unwrap(), 0),
                case["tree"],
                "source DOM {}",
                case["id"]
            );
        }
    }
    assert_eq!(fixture["rows"].as_array().unwrap().len(), 61);
    assert_eq!(limits, 18);
}

#[test]
fn current_ruby_facade_recovery_and_scalar_boundaries() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../fixtures/xml-parse/recovery-current-models.json"
    ))
    .unwrap();
    for case in fixture["rows"].as_array().unwrap() {
        let actual = parse(
            case["input"].as_str().unwrap(),
            case["format"].as_str().unwrap(),
            &json!({}),
        );
        if let Some(limit) = case.get("native_policy_limit") {
            assert!(actual.unwrap_err()["message"]
                .as_str()
                .unwrap()
                .contains(limit["message"].as_str().unwrap()));
        } else if case.get("error").is_some() {
            assert!(actual.is_err(), "source refusal {}", case["id"]);
        } else {
            assert_eq!(
                actual.unwrap(),
                case["model"],
                "current source {}",
                case["id"]
            );
        }
    }
    assert_eq!(fixture["rows"].as_array().unwrap().len(), 17);
}
