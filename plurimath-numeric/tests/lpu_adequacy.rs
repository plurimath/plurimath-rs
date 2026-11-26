use serde_json::{json, Value};

fn var(n: &str) -> Value {
    json!({"type":"variable","name":n})
}
fn num(t: &str) -> Value {
    json!({"type":"number","text":t})
}
fn bin(op: &str, l: Value, r: Value) -> Value {
    json!({"type":"binary","operator":op,"left":l,"right":r})
}
fn call(n: &str, a: Value) -> Value {
    json!({"type":"call","name":n,"argument":a})
}

fn run(parameters: Value) -> Value {
    let req = json!({"schema":"plurimath.evaluate.gum/1","method":"lpu_adequacy","parameters":parameters});
    serde_json::from_str(&plurimath_numeric::evaluate_json(&req.to_string())).unwrap()
}

fn params(ast: Value, budget: Value) -> Value {
    json!({
        "ast": ast,
        "inputs":[{"name":"x","estimate":10.0,"uncertainty":0.1}],
        "coverage_factor": 2.0, "threshold": 0.05, "budget": budget
    })
}

fn linear() -> Value {
    bin("+", bin("*", num("3"), var("x")), num("1"))
}

#[test]
fn linear_model_is_adequate_and_budget_is_echoed() {
    let out = run(params(linear(), json!(64)));
    assert_eq!(out["ok"], true, "{out}");
    let r = &out["result"];
    assert_eq!(r["verdict"], "adequate");
    assert_eq!(r["subdivision"]["budget"], 64);
    assert_eq!(r["subdivision"]["boxes_used"], 1);
    assert_eq!(r["threshold"], 0.05);
}

#[test]
fn sin_model_is_refused_as_unsupported() {
    let out = run(params(call("sin", var("x")), json!(64)));
    assert_eq!(out["ok"], false, "{out}");
    assert_eq!(out["error"]["code"], "UnsupportedModel");
}

#[test]
fn missing_required_parameter_is_refused() {
    for key in ["ast", "inputs", "coverage_factor", "threshold", "budget"] {
        let mut p = params(linear(), json!(64));
        p.as_object_mut().unwrap().remove(key);
        let out = run(p);
        assert_eq!(out["ok"], false, "{key}: {out}");
        assert_eq!(out["error"]["code"], "InvalidInput", "{key}: {out}");
    }
}

#[test]
fn budget_outside_range_is_refused() {
    for budget in [json!(0), json!(100001), json!(-1), json!(1.5), json!("64")] {
        let out = run(params(linear(), budget.clone()));
        assert_eq!(out["ok"], false, "{budget}: {out}");
        assert_eq!(out["error"]["code"], "InvalidInput", "{budget}: {out}");
    }
    assert_eq!(run(params(linear(), json!(1)))["ok"], true);
    assert_eq!(run(params(linear(), json!(100000)))["ok"], true);
}
