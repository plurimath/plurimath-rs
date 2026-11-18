use plurimath_facade::writer;
use serde_json::{json, Value};

fn sym(value: &str) -> Value {
    json!({"class":"Math::Symbols::Symbol","fields":{"value":value}})
}

fn formula(value: Value) -> Value {
    json!({"class":"Math::Formula","fields":{"value":[value]}})
}

fn write(model: Value, format: &str) -> Result<String, String> {
    writer::write(&formula(model), format, &json!({}))
}

fn menclose(notation: Value, body: Value) -> Value {
    json!({"class":"Math::Function::Menclose","fields":{"parameter_one":notation,"parameter_two":body}})
}

#[test]
fn menclose_text_writers() {
    let m = || menclose(json!("longdiv"), sym("x"));
    assert_eq!(write(m(), "asciimath").unwrap(), "x");
    assert_eq!(write(m(), "latex").unwrap(), "x");
    assert_eq!(write(m(), "unicodemath").unwrap(), "⟌(x)");
    assert_eq!(
        write(menclose(json!("left right"), sym("x")), "unicodemath").unwrap(),
        "▭(3&x)"
    );
    assert_eq!(
        write(menclose(json!("actuarial"), sym("x")), "unicodemath").unwrap(),
        ""
    );
    assert!(write(menclose(Value::Null, sym("x")), "unicodemath").is_err());
}

#[test]
fn menclose_xml_writers() {
    let mathml = write(menclose(json!("box"), sym("x")), "mathml").unwrap();
    assert!(mathml.contains("<menclose notation=\"box\">"), "{mathml}");
    let omml = write(menclose(json!("box"), sym("x")), "omml").unwrap();
    assert!(omml.contains("<m:borderBox>"), "{omml}");
    assert!(!omml.contains("m:borderBoxPr"), "{omml}");
    let omml = write(menclose(json!("top horizontalstrike"), sym("x")), "omml").unwrap();
    for prop in ["hideBot", "hideLeft", "hideRight", "strikeH"] {
        assert!(
            omml.contains(&format!("<m:{prop} m:val=\"on\"/>")),
            "{omml}"
        );
    }
    assert!(!omml.contains("hideTop"), "{omml}");
    assert!(write(menclose(Value::Null, sym("x")), "omml").is_err());
}

#[test]
fn overleftrightarrow_writers() {
    let node = || json!({"class":"Math::Function::Overleftrightarrow","fields":{"attributes":{},"parameter_one":sym("x")}});
    assert_eq!(write(node(), "asciimath").unwrap(), "overleftrightarrow(x)");
    assert_eq!(write(node(), "latex").unwrap(), "\\overleftrightarrow{x}");
    assert_eq!(write(node(), "unicodemath").unwrap(), "(x)\u{20e1}");
    let mathml = write(node(), "mathml").unwrap();
    assert!(mathml.contains("<mover>"), "{mathml}");
    let omml = write(node(), "omml").unwrap();
    assert!(omml.contains("m:limUpp"), "{omml}");
}

#[test]
fn semantics_writers() {
    let node = || {
        json!({"class":"Math::Function::Semantics","fields":{
            "parameter_one":sym("x"),
            "parameter_two":[{"annotation":[sym("y")]}]
        }})
    };
    assert_eq!(write(node(), "latex").unwrap(), "x");
    let mathml = write(node(), "mathml").unwrap();
    assert!(mathml.contains("<semantics>"), "{mathml}");
    assert!(mathml.contains("<annotation>"), "{mathml}");
    assert!(write(node(), "html").is_err());
}
