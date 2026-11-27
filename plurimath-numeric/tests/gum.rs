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

fn gum_par_request(case: usize, threads: u64) -> Value {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-mcm.json")).unwrap();
    let mut request = fixture["cases"][case]["request"].clone();
    let parameters = request["parameters"].as_object_mut().unwrap();
    parameters.remove("seeds");
    parameters.insert("engine".into(), json!("gum-par"));
    parameters.insert("seed".into(), json!(20261124));
    parameters.insert("threads".into(), json!(threads));
    request
}

#[test]
fn gum_par_is_identical_across_thread_counts_and_records_counter_ranges() {
    let single = run(&gum_par_request(4, 1));
    assert_eq!(single["ok"], true, "{single}");
    for threads in [2, 3, 8] {
        assert_eq!(
            run(&gum_par_request(4, threads)),
            single,
            "threads={threads}"
        );
    }
    let engine = &single["result"]["engine"];
    assert_eq!(engine["id"], "gum-par");
    assert_eq!(engine["accuracy"], "limited");
    let ranges = engine["counter_ranges"].as_array().unwrap();
    assert_eq!(
        ranges.len() as u64,
        single["result"]["batches"].as_u64().unwrap()
    );
    assert_eq!(ranges[1]["first"], engine["counter_stride"]);
    assert_eq!(single["result"]["stabilized"], true);
}

#[test]
fn gum_par_engine_selection_is_validated() {
    let mut both = gum_par_request(4, 1);
    both["parameters"]["seeds"] = json!([12345, 23456, 34567, 45678]);
    assert_eq!(run(&both)["error"]["code"], "InvalidInput");
    let mut unknown = gum_par_request(4, 1);
    unknown["parameters"]["engine"] = json!("other");
    assert_eq!(run(&unknown)["ok"], false);
}

fn gum_ref_request() -> Value {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-mcm.json")).unwrap();
    fixture["cases"][4]["request"].clone()
}

fn with_provenance(mut request: Value, flag: Value) -> Value {
    request["parameters"]["provenance"] = flag;
    request
}

const ACCURACY_CLASSES: [&str; 2] = ["limited", "limited_float"];

fn assert_provenance_complete(engine: &Value, id: &str) {
    let provenance = &engine["provenance"];
    assert_eq!(provenance["engine_id"], id, "{provenance}");
    assert!(provenance["device"].is_null(), "{provenance}");
    let rng = &provenance["rng"];
    assert!(rng["family"].is_string() && rng["uniform_draws"].is_u64());
    let accuracy = provenance["accuracy"].as_object().unwrap();
    for key in [
        "estimate",
        "standard_uncertainty",
        "intervals",
        "twice_standard_errors",
    ] {
        assert!(ACCURACY_CLASSES.contains(&accuracy[key].as_str().unwrap()));
    }
    if id == "gum-par" {
        assert_eq!(rng["family"], "philox4x32-10");
        assert!(rng["counter_ranges"].is_array() && rng["seed"].is_u64());
        assert!(provenance["reduction"].is_string());
    } else {
        assert!(rng["initial_seeds"].is_array() && rng["final_state"].is_array());
        assert!(provenance["reduction"].is_null());
    }
}

#[test]
fn provenance_is_always_present_for_gum_par() {
    let response = run(&gum_par_request(4, 2));
    assert_eq!(response["ok"], true, "{response}");
    assert_provenance_complete(&response["result"]["engine"], "gum-par");
}

#[test]
fn provenance_is_opt_in_for_gum_ref_and_frozen_methods() {
    let base = run(&gum_ref_request());
    assert!(base["result"]["engine"].get("provenance").is_none());
    let on = run(&with_provenance(gum_ref_request(), json!(true)));
    assert_eq!(on["ok"], true, "{on}");
    assert_provenance_complete(&on["result"]["engine"], "gum-ref");
    let off = run(&with_provenance(gum_ref_request(), json!(false)));
    assert_eq!(off, base);
    let bad = run(&with_provenance(gum_ref_request(), json!("yes")));
    assert_eq!(bad["ok"], false);
    // A deterministic frozen method: opt-in adds the object with null rng.
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-f40.json")).unwrap();
    let request = fixture["cases"][0]["request"].clone();
    let plain = run(&request);
    assert_eq!(plain["ok"], true, "{plain}");
    let on = run(&with_provenance(request, json!(true)));
    assert_eq!(on["result"]["engine"]["provenance"]["rng"], Value::Null);
    assert_eq!(on["result"]["engine"]["provenance"]["engine_id"], "gum-ref");
}

/// A /1 consumer reads only known fields. With provenance present, every
/// pre-existing field of the response must be unchanged.
#[test]
fn legacy_consumer_reads_same_fields_with_and_without_provenance() {
    let mut cases = vec![gum_ref_request()];
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/gum-f40.json")).unwrap();
    cases.push(fixture["cases"][0]["request"].clone());
    for request in cases {
        let plain = run(&request);
        let mut extended = run(&with_provenance(request, json!(true)));
        assert_eq!(plain["schema"], "plurimath.evaluate.gum/1");
        assert_eq!(extended["schema"], plain["schema"]);
        extended["result"]["engine"]
            .as_object_mut()
            .unwrap()
            .remove("provenance")
            .expect("provenance present");
        assert_eq!(extended, plain);
    }
}

fn fixture_request(source: &str, case: usize) -> Value {
    let fixture: Value = serde_json::from_str(source).unwrap();
    fixture["cases"][case]["request"].clone()
}

fn accuracy_of(request: &Value) -> Value {
    let plain = run(request);
    assert_eq!(plain["ok"], true, "{plain}");
    assert!(plain["result"]["engine"].get("provenance").is_none());
    assert_eq!(run(&with_provenance(request.clone(), json!(false))), plain);
    let on = run(&with_provenance(request.clone(), json!(true)));
    assert_eq!(on["ok"], true, "{on}");
    let provenance = &on["result"]["engine"]["provenance"];
    assert!(provenance["rng"].is_null(), "{provenance}");
    provenance["accuracy"].clone()
}

#[test]
fn accuracy_is_derived_from_value_tags_for_every_deterministic_method() {
    const F40: &str = include_str!("../fixtures/gum-f40.json");
    const ANNEX: &str = include_str!("../fixtures/gum-annex-methods.json");
    const CONFORMITY: &str = include_str!("../fixtures/gum-conformity.json");
    const VALIDATION: &str = include_str!("../fixtures/coverage-validation.json");
    // rational => exact; decimal => limited_<guard_digits>; float => limited_float.
    let cases = [
        (
            F40,
            5,
            "type_a",
            json!({
            "mean":"exact","minimum":"exact","maximum":"exact",
            "sample_variance":"exact","variance":"exact"}),
        ),
        (F40, 10, "component", json!({"variance":"exact"})),
        (
            F40,
            19,
            "series",
            json!({
            "mean":"exact","minimum":"exact","maximum":"exact",
            "sample_variance":"exact","variance":"exact",
            "sample_standard_deviation":"limited_12",
            "standard_uncertainty_of_mean":"limited_12"}),
        ),
        (
            F40,
            6,
            "grouped_type_a",
            json!({
            "mean":"exact","within_group_variance":"exact","group_mean_variance":"exact",
            "between_group_component":"exact","between_mean_square":"exact",
            "within_mean_square":"exact","all_observations_sample_variance":"exact",
            "f_statistic":"exact","variance":"exact","standard_uncertainty":"limited_50"}),
        ),
        (
            F40,
            23,
            "coverage",
            json!({
            "coverage_factor":"limited_float","expanded_uncertainty":"limited_float",
            "probability":"exact"}),
        ),
        (
            F40,
            1,
            "propagate_vector",
            json!({
            "estimates":"exact","covariance":"exact","standard_uncertainties":"limited_50"}),
        ),
        (
            ANNEX,
            19,
            "type_a_covariance",
            json!({
            "means":"exact","sample_covariance":"exact","covariance_of_mean":"exact"}),
        ),
        (
            ANNEX,
            0,
            "linear_calibration",
            json!({
            "intercept":"exact","slope":"exact","covariance":"exact",
            "reference_temperature":"exact","temperature_range":"exact","residuals":"exact",
            "residual_variance":"exact","residual_standard_deviation":"limited_40"}),
        ),
        (
            ANNEX,
            2,
            "predict_calibration",
            json!({
            "estimate":"exact","variance":"exact","standard_uncertainty":"limited_40"}),
        ),
        (
            ANNEX,
            10,
            "propagate_implicit",
            json!({
            "estimates":"exact","covariance":"exact","standard_uncertainties":"limited_40",
            "input_sensitivities":"exact","root_residuals":"exact","residual_tolerance":"exact"}),
        ),
        (
            ANNEX,
            14,
            "analyze",
            json!({
            "estimate":"exact","variance":"exact","standard_uncertainty":"limited_50",
            "gradient":"exact","hessian":"exact","third":"exact"}),
        ),
        (
            ANNEX,
            20,
            "propagate_covariance",
            json!({
            "estimate":"exact","variance":"exact","standard_uncertainty":"limited_50"}),
        ),
        (
            VALIDATION,
            0,
            "validate_coverage",
            json!({
            "probability":"exact","tolerance":"exact","monte_carlo_tolerance":"exact",
            "endpoint_differences":"exact"}),
        ),
        (
            CONFORMITY,
            0,
            "conformity",
            json!({
            "estimate":"exact","standard_uncertainty":"exact","coverage_factor":"exact",
            "guard_ratio":"exact","conformance_probability":"limited_float",
            "nonconformance_probability":"limited_float",
            "specific_consumer_risk":"limited_float",
            "maximum_specific_consumer_risk_if_accepted":"limited_float",
            "tolerance":"exact","acceptance_interval":"exact","signed_guard_band":"exact",
            "expanded_uncertainty":"exact","lower_residual":"exact",
            "probability_resolution":"exact"}),
        ),
    ];
    for (source, case, method, expected) in cases {
        let request = fixture_request(source, case);
        assert_eq!(request["method"], method);
        assert_eq!(accuracy_of(&request), expected, "{method}");
    }
}

#[test]
fn accuracy_covers_propagate_and_the_float_only_methods() {
    let propagate = accuracy_of(&request(
        "propagate",
        json!({
            "estimate":10,"components":[{"variance":4,"degrees_of_freedom":8},{"variance":9}],
            "gradient":[1,2],"independent":true,"guard_digits":40,
        }),
    ));
    assert_eq!(propagate["variance"], "exact");
    assert_eq!(propagate["standard_uncertainty"], "limited_40");

    let ast = json!({"type":"binary","operator":"*",
        "left":{"type":"variable","name":"x"},"right":{"type":"variable","name":"y"}});
    let inputs = json!([
        {"name":"x","estimate":3.0,"uncertainty":0.5},
        {"name":"y","estimate":2.0,"uncertainty":0.25}]);
    let ad = accuracy_of(&request("analyze_ad", json!({"ast":ast,"inputs":inputs})));
    for key in ["value", "gradient", "hessian", "first_order_variance"] {
        assert_eq!(ad[key], "limited_float", "{key}");
    }
    let adequacy = accuracy_of(&request(
        "lpu_adequacy",
        json!({"ast":ast,"inputs":inputs,"coverage_factor":2.0,"threshold":0.05,"budget":64}),
    ));
    for key in ["error_enclosure", "u_c_upper", "ratio_upper", "threshold"] {
        assert_eq!(adequacy[key], "limited_float", "{key}");
    }
    assert!(adequacy.get("verdict").is_none());
}

#[test]
fn accuracy_of_a_bare_matrix_result_is_left_unchanged() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/gum-annex-methods.json")).unwrap();
    let request = fixture["cases"][4]["request"].clone();
    assert_eq!(request["method"], "correlation_matrix");
    let plain = run(&request);
    assert_eq!(plain["ok"], true, "{plain}");
    assert_eq!(run(&with_provenance(request, json!(true))), plain);
}

#[test]
fn sampled_methods_keep_their_monte_carlo_accuracy_map() {
    let on = run(&with_provenance(gum_ref_request(), json!(true)));
    let accuracy = &on["result"]["engine"]["provenance"]["accuracy"];
    assert_eq!(accuracy["estimate"], "limited_float");
    assert_eq!(accuracy["intervals"], "limited_float");
}

const GUM_FIXTURES: [&str; 6] = [
    include_str!("../fixtures/gum-f40.json"),
    include_str!("../fixtures/gum-annex-methods.json"),
    include_str!("../fixtures/gum-conformity.json"),
    include_str!("../fixtures/gum-mcm.json"),
    include_str!("../fixtures/gum-mcm-joint-rational.json"),
    include_str!("../fixtures/coverage-validation.json"),
];

/// Every gum/1 request in the frozen fixtures that succeeds with an object
/// result, as `(request, plain response)`.
fn successful_object_cases() -> Vec<(Value, Value)> {
    let mut found = Vec::new();
    for source in GUM_FIXTURES {
        let fixture: Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let request = &case["request"];
            if request["schema"] != "plurimath.evaluate.gum/1" {
                continue;
            }
            let plain = run(request);
            if plain["ok"] == true && plain["result"].is_object() {
                found.push((request.clone(), plain));
            }
        }
    }
    found
}

fn assert_certificate_shape(provenance: &Value, label: &str) {
    let keys: Vec<&str> = provenance
        .as_object()
        .unwrap_or_else(|| panic!("{label}: provenance is not an object"))
        .keys()
        .map(String::as_str)
        .collect();
    for key in ["engine_id", "rng", "reduction", "accuracy", "device"] {
        assert!(keys.contains(&key), "{label}: missing {key}: {provenance}");
    }
    assert_eq!(keys.len(), 5, "{label}: unexpected members: {provenance}");
    assert!(provenance["device"].is_null(), "{label}");
    assert!(
        provenance["accuracy"].is_object(),
        "{label}: accuracy must be a map: {provenance}"
    );
    for (field, class) in provenance["accuracy"].as_object().unwrap() {
        let class = class.as_str().unwrap_or("");
        assert!(
            class == "exact" || class.starts_with("limited"),
            "{label}: accuracy class of {field} is {class:?}"
        );
    }
}

/// Contract: with provenance requested, every method of every engine returns
/// a complete certificate (engine id, rng, reduction, accuracy map, device).
#[test]
fn provenance_certificate_is_complete_for_every_method_and_engine() {
    let cases = successful_object_cases();
    assert!(cases.len() > 50, "only {} cases found", cases.len());
    let mut sampled = 0;
    for (index, (request, plain)) in cases.iter().enumerate() {
        let label = format!(
            "case {index} method {}",
            request["method"].as_str().unwrap_or("?")
        );
        let on = run(&with_provenance(request.clone(), json!(true)));
        assert_eq!(on["ok"], true, "{label}: {on}");
        let engine = &on["result"]["engine"];
        let provenance = &engine["provenance"];
        assert_certificate_shape(provenance, &label);
        assert_eq!(provenance["engine_id"], "gum-ref", "{label}");
        if plain["result"]["engine"].get("generator").is_some() {
            sampled += 1;
            let rng = &provenance["rng"];
            assert!(rng["family"].is_string(), "{label}: {rng}");
            assert!(provenance["accuracy"]["estimate"].is_string(), "{label}");
        } else {
            assert!(provenance["rng"].is_null(), "{label}");
        }
    }
    assert!(sampled > 0, "no sampled (Monte Carlo) case was exercised");
    // gum-par stamps itself without being asked and carries counter ranges.
    for request in [
        gum_par_request(4, 1),
        with_provenance(gum_par_request(4, 2), json!(true)),
    ] {
        let response = run(&request);
        assert_eq!(response["ok"], true, "{response}");
        let provenance = &response["result"]["engine"]["provenance"];
        assert_certificate_shape(provenance, "gum-par");
        assert_eq!(provenance["engine_id"], "gum-par");
        assert!(provenance["rng"]["counter_ranges"].is_array());
        assert!(provenance["reduction"].is_string());
    }
}

/// Additive evolution: a gum/1 consumer that never sets `provenance` gets
/// the same bytes as before for every gum-ref method, still under /1, and an
/// explicit `false` changes nothing. Turning it on only adds
/// `engine.provenance`.
#[test]
fn gum_1_consumer_without_provenance_sees_unchanged_schema() {
    for (index, (request, plain)) in successful_object_cases().iter().enumerate() {
        let label = format!(
            "case {index} method {}",
            request["method"].as_str().unwrap_or("?")
        );
        assert_eq!(plain["schema"], "plurimath.evaluate.gum/1", "{label}");
        if plain["result"]["engine"]["id"] == "gum-par" {
            continue;
        }
        assert!(
            plain["result"]["engine"].get("provenance").is_none(),
            "{label}: provenance leaked into a /1 response"
        );
        let raw = evaluate_json(&request.to_string());
        let off = evaluate_json(&with_provenance(request.clone(), json!(false)).to_string());
        assert_eq!(raw, off, "{label}: provenance=false changed the bytes");
        let mut on = run(&with_provenance(request.clone(), json!(true)));
        assert_eq!(on["schema"], plain["schema"], "{label}");
        on["result"]["engine"]
            .as_object_mut()
            .unwrap()
            .remove("provenance")
            .unwrap_or_else(|| panic!("{label}: provenance missing when requested"));
        // A result with no engine stamp gains one only to hold the certificate.
        if plain["result"].get("engine").is_none() {
            let empty = on["result"]["engine"]
                .as_object()
                .is_some_and(|e| e.is_empty());
            assert!(empty, "{label}: engine gained members besides provenance");
            on["result"].as_object_mut().unwrap().remove("engine");
        }
        assert_eq!(&on, plain, "{label}: provenance changed a /1 field");
    }
}
