use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};

fn run(request: &Value) -> Value {
    serde_json::from_str(&evaluate_json(&request.to_string())).unwrap()
}

fn request(method: &str, params: Value) -> Value {
    json!({"schema":"plurimath.evaluate.gum/1","method":method,"parameters":params})
}

fn rational(numerator: &str, denominator: &str) -> Value {
    json!({"rational":[numerator,denominator]})
}

fn project_engine(value: &mut Value) {
    if let Some(entries) = value.as_array_mut() {
        entries.iter_mut().for_each(project_engine);
    }
    if let Some(engine) = value.get_mut("engine") {
        engine.as_object_mut().unwrap().remove("name");
        engine.as_object_mut().unwrap().remove("version");
    }
}

#[test]
fn frozen_f40_gum_and_series_cases() {
    assert_frozen(include_str!("../fixtures/gum-f40.json"), 39);
}

#[test]
fn frozen_conformity_and_ruby_edge_cases() {
    assert_frozen(include_str!("../fixtures/gum-conformity.json"), 25);
}

fn assert_frozen(source: &str, count: usize) {
    let fixture: Value = serde_json::from_str(source).unwrap();
    let mut failures = Vec::new();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), count);
    for case in cases {
        let actual = run(&case["request"]);
        let expected = &case["expected"];
        if expected["outcome"] == "ok" {
            let mut got = actual["result"].clone();
            let mut want = expected["result"].clone();
            project_engine(&mut got);
            project_engine(&mut want);
            if !actual["ok"].as_bool().unwrap() {
                failures.push(format!(
                    "{} unexpected error: {}",
                    case["id"], actual["error"]
                ));
            } else if got != want {
                let differing: Vec<_> = match (got.as_object(), want.as_object()) {
                    (Some(actual), Some(expected)) => expected
                        .iter()
                        .filter(|(k, v)| actual.get(*k) != Some(*v))
                        .map(|(k, v)| {
                            format!(
                                "{k}: {} expected {v}",
                                actual.get(k).unwrap_or(&Value::Null)
                            )
                        })
                        .collect(),
                    _ => vec![format!("{got} expected {want}")],
                };
                failures.push(format!("{} {}", case["id"], differing.join("; ")));
            }
        } else {
            let class = expected["error_class"]
                .as_str()
                .unwrap()
                .rsplit("::")
                .next()
                .unwrap();
            if actual["ok"] != false
                || actual["error"]["code"] != class
                || actual["error"]["message"] != expected["message"]
            {
                failures.push(format!(
                    "{} {} expected {}: {}",
                    case["id"], actual["error"], class, expected["message"]
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn ruby_spec_lpu_and_exact_singular_covariance() {
    let independent = run(&request(
        "propagate",
        json!({
            "estimate":10,"components":[{"variance":4,"degrees_of_freedom":8},{"variance":9}],
            "gradient":[1,2],"independent":true,"guard_digits":40,
        }),
    ));
    assert_eq!(independent["result"]["variance"], rational("40", "1"));
    assert_eq!(
        independent["result"]["effective_degrees_of_freedom"],
        rational("800", "1")
    );
    assert_eq!(
        independent["result"]["adequacy"],
        json!({"symbol":"not_assessed"})
    );
    let singular = run(&request(
        "propagate_covariance",
        json!({
            "estimate":0,"components":[{"variance":"1/3"},{"variance":"4/3"}],
            "gradient":[2,-1],"covariance":[["1/3","2/3"],["2/3","4/3"]],
            "joint_distribution":"unspecified","guard_digits":40,
        }),
    ));
    assert_eq!(singular["result"]["variance"], rational("0", "1"));
    assert_eq!(
        singular["result"]["degrees_of_freedom_status"],
        json!({"symbol":"not_evaluated_for_correlated_inputs"})
    );
    for matrix in [
        json!([[1, 2], [2, 1]]),
        json!([[0, 1], [1, 1]]),
        json!([[1, 1], [0, 1]]),
    ] {
        let refused = run(&request(
            "propagate_vector",
            json!({
                "estimates":[0],"jacobian":[[1,1]],"covariance":matrix,"guard_digits":40,
            }),
        ));
        assert_eq!(refused["error"]["code"], "InvalidInput");
    }
}

#[test]
fn annex_h1_analytic_derivatives_and_declared_pdf_lpu() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-annex.json")).unwrap();
    let case = &fixture["h1"];
    let response = run(&case["request"]);
    assert_eq!(response["ok"], true, "{}", response["error"]);
    for (key, value) in case["expected"].as_object().unwrap() {
        assert_eq!(&response["result"][key], value, "{key}");
    }
    assert_eq!(case["actual_pdf_normal"], false);
    let mut unsupported = case["request"].clone();
    unsupported["parameters"]["order"] = json!(2);
    let refusal = run(&unsupported);
    assert_eq!(refusal["error"]["code"], "UnsupportedModel");
    assert_eq!(
        refusal["error"]["message"],
        "GUF2 requires normal inputs; use distribution propagation"
    );
}

#[test]
fn exact_second_order_contractions_and_typed_analytic_refusals() {
    let model = json!({
        "type":"binary","operator":"+","left":{"type":"variable","name":"x"},
        "right":{"type":"binary","operator":"^","left":{"type":"variable","name":"x"},
            "right":{"type":"number","text":"3"}},
    });
    let inputs =
        json!([{"name":"x","estimate":0,"component":{"variance":"1/100","distribution":"normal"}}]);
    let mut analyze = request(
        "analyze",
        json!({
            "ast":model,"inputs":inputs,"order":2,"independent":true,"guard_digits":40,
        }),
    );
    let result = run(&analyze);
    assert_eq!(result["result"]["variance"], rational("53", "5000"));
    assert_eq!(result["result"]["third"], json!([[[rational("6", "1")]]]));
    analyze["parameters"]["ast"] =
        json!({"type":"call","name":"sin","argument":{"type":"variable","name":"x"}});
    assert_eq!(run(&analyze)["error"]["code"], "UnsupportedModel");
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-annex.json")).unwrap();
    for case in fixture["joint_gaussian"].as_array().unwrap() {
        let output = run(&case["request"]);
        for (key, value) in case["expected"].as_object().unwrap() {
            assert_eq!(&output["result"][key], value, "{} {key}", case["id"]);
        }
        let mut refusal = case["request"].clone();
        refusal["parameters"]["joint_distribution"] = json!("unspecified");
        assert_eq!(run(&refusal)["error"]["code"], "UnsupportedModel");
    }
}

#[test]
fn sealed_monte_carlo_profiles_preserve_intervals_rng_and_adaptive_stops() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-mcm.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    let mut failures = Vec::new();
    for case in cases {
        let out = run(&case["request"]);
        if out["ok"] != true {
            failures.push(format!("{} {}", case["id"], out["error"]));
            continue;
        }
        let mut actual = out["result"].clone();
        let mut expected = case["expected"].clone();
        for key in ["method", "precision_convention"] {
            expected[key] = json!({"symbol":expected[key]});
        }
        project_engine(&mut actual);
        project_engine(&mut expected);
        for (key, value) in expected.as_object().unwrap() {
            if &actual[key] != value {
                failures.push(format!(
                    "{} {key}: actual {} expected {value}",
                    case["id"], actual[key]
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn monte_carlo_refuses_undeclared_joint_laws_and_nonpolynomial_models() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-mcm.json")).unwrap();
    let mut joint = fixture["cases"][2]["request"].clone();
    joint["parameters"]["independent"] = json!(false);
    assert_eq!(run(&joint)["error"]["code"], "UnsupportedModel");
    let mut rational = fixture["cases"][2]["request"].clone();
    rational["parameters"]["ast"] = json!({
        "type":"binary","operator":"/","left":{"type":"number","text":"1"},
        "right":{"type":"binary","operator":"+","left":{"type":"variable","name":"a"},"right":{"type":"number","text":"1"}},
    });
    assert_eq!(run(&rational)["error"]["code"], "UnsupportedModel");
}

#[test]
fn published_joint_and_rational_monte_carlo_profiles() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/gum-mcm-joint-rational.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 7);
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let actual = run(&case["request"]);
        if actual["ok"] != true {
            failures.push(format!("{} {}", case["id"], actual["error"]));
            continue;
        }
        let mut actual = actual["result"].clone();
        let mut expected = case["expected"].clone();
        for key in ["method", "precision_convention"] {
            expected[key] = json!({"symbol":expected[key]});
        }
        if let Some(law) = expected["engine"].get("joint_distribution").cloned() {
            expected["engine"]["joint_distribution"] = json!({"symbol":law});
        }
        project_engine(&mut actual);
        project_engine(&mut expected);
        for (key, want) in expected.as_object().unwrap() {
            if &actual[key] != want {
                failures.push(format!(
                    "{} {key}: {} expected {want}",
                    case["id"], actual[key]
                ));
            }
        }
        eprintln!("completed source profile {}", case["id"]);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn ruby_annex_calibration_and_implicit_cases() {
    assert_frozen(include_str!("../fixtures/gum-annex-methods.json"), 22);
}

#[test]
fn source_coverage_metadata_diagnostics() {
    assert_frozen(include_str!("../fixtures/coverage-validation.json"), 49);
}
