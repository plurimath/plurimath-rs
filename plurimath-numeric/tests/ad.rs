use plurimath_numeric::ad::{analyze, hessian_diagonal, sensitivities, Input};
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
fn inp<'a>(name: &'a str, estimate: f64, uncertainty: f64) -> Input<'a> {
    Input {
        name,
        estimate,
        uncertainty,
    }
}
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * b.abs().max(1.0), "{a} vs {b}");
}

/// f = x^2 y + sin(x) / y : known partials.
fn model() -> Value {
    bin(
        "+",
        bin("*", bin("^", var("x"), num("2")), var("y")),
        bin("/", call("sin", var("x")), var("y")),
    )
}

#[test]
fn known_answer_gradient_and_hessian_diagonal() {
    let (x, y) = (0.7_f64, 1.9_f64);
    let inputs = [inp("x", x, 0.1), inp("y", y, 0.2)];
    let r = analyze(&model(), &inputs).unwrap();
    assert_eq!(r.class, "limited");
    close(r.value, x * x * y + x.sin() / y, 1e-15);
    close(r.gradient[0], 2.0 * x * y + x.cos() / y, 1e-14);
    close(r.gradient[1], x * x - x.sin() / (y * y), 1e-14);
    close(r.hessian[0][0], 2.0 * y - x.sin() / y, 1e-14);
    close(r.hessian[1][1], 2.0 * x.sin() / (y * y * y), 1e-14);
    let fxy = 2.0 * x - x.cos() / (y * y);
    close(r.hessian[0][1], fxy, 1e-14);
    close(r.hessian[1][0], fxy, 1e-14);
    let (hxx, hyy) = (r.hessian[0][0], r.hessian[1][1]);
    let expected = 0.5 * (hxx * hxx * 1e-4 + hyy * hyy * 16e-4 + 2.0 * fxy * fxy * 0.01 * 0.04);
    close(r.second_order_term, expected, 1e-13);
}

#[test]
fn matches_central_finite_difference_references() {
    let f = |x: f64, y: f64| x * x * y + x.sin() / y;
    let (x, y, h) = (0.7_f64, 1.9_f64, 1e-4);
    let inputs = [inp("x", x, 1.0), inp("y", y, 1.0)];
    let g = sensitivities(&model(), &inputs).unwrap();
    let hd = hessian_diagonal(&model(), &inputs).unwrap();
    close(g[0], (f(x + h, y) - f(x - h, y)) / (2.0 * h), 1e-7);
    close(g[1], (f(x, y + h) - f(x, y - h)) / (2.0 * h), 1e-7);
    close(
        hd[0],
        (f(x + h, y) - 2.0 * f(x, y) + f(x - h, y)) / (h * h),
        1e-5,
    );
    close(
        hd[1],
        (f(x, y + h) - 2.0 * f(x, y) + f(x, y - h)) / (h * h),
        1e-5,
    );
}

/// Closed forms: f = a b has f_ab = 1; f = x^3 has f_xx = 6x.
#[test]
fn second_order_term_closed_forms() {
    let r = analyze(
        &bin("*", var("a"), var("b")),
        &[inp("a", 2.0, 0.5), inp("b", 3.0, 0.5)],
    )
    .unwrap();
    assert_eq!(r.hessian, vec![vec![0.0, 1.0], vec![1.0, 0.0]]);
    // 1/2 (f_ab^2 ua^2 ub^2 + f_ba^2 ub^2 ua^2) = ua^2 ub^2
    close(r.second_order_term, 0.0625, 1e-15);
    let cube = analyze(&bin("^", var("x"), num("3")), &[inp("x", 2.0, 0.1)]).unwrap();
    close(cube.hessian[0][0], 12.0, 1e-15);
    close(cube.second_order_term, 0.5 * 144.0 * 1e-4, 1e-14);
    close(
        cube.first_order_variance,
        (3.0 * 4.0_f64).powi(2) * 0.01,
        1e-15,
    );
}

#[test]
fn exp_ln_sqrt_general_power_and_errors() {
    let x = 1.3_f64;
    let m = bin("*", call("exp", var("x")), call("ln", var("x")));
    let r = analyze(&m, &[inp("x", x, 1.0)]).unwrap();
    close(r.gradient[0], x.exp() * (x.ln() + 1.0 / x), 1e-14);
    close(
        r.hessian[0][0],
        x.exp() * (x.ln() + 2.0 / x - 1.0 / (x * x)),
        1e-14,
    );
    let s = analyze(&bin("^", var("x"), num("0.5")), &[inp("x", 4.0, 1.0)]).unwrap();
    close(s.gradient[0], 0.25, 1e-14);
    close(s.hessian[0][0], -1.0 / 32.0, 1e-14);
    let err = analyze(&bin("/", num("1"), var("x")), &[inp("x", 0.0, 1.0)]);
    assert_eq!(err.unwrap_err(), "DivisionByZeroError");
    assert!(analyze(&call("tan", var("x")), &[inp("x", 1.0, 1.0)]).is_err());
}

/// JCGM 100:2008 H.1 (end gauge, model H.3) with inputs as printed in H.1.3:
/// lS = 0.05 m, u(lS) = 25 nm, u(d) = 9.7 nm, u(aS) = 1.2e-6 /C, u(theta) = 0.41 C,
/// u(d_alpha) = 0.58e-6 /C, u(d_theta) = 0.029 C; theta = -0.1 C.
/// Printed: u_c = 32 nm (H.6c); second-order terms lS u(da) u(th) = 11.7 nm and
/// lS u(aS) u(dth) = 1.7 nm, raising u_c from 32 nm to 34 nm (H.1.7, GUM PDF p. 97).
/// JCGM 101:2008 9.5.4 Table 11 (printed p. 55): GUF u = 32 nm, MCM u = 36 nm.
fn end_gauge() -> (Value, Vec<Input<'static>>) {
    let l = bin(
        "*",
        var("lS"),
        bin(
            "+",
            bin("*", var("da"), var("th")),
            bin("*", var("aS"), var("dth")),
        ),
    );
    let ast = bin("-", bin("+", var("lS"), var("d")), l);
    let inputs = vec![
        inp("lS", 0.05, 25e-9),
        inp("d", 215e-9, 9.7e-9),
        inp("aS", 11.5e-6, 1.2e-6),
        inp("th", -0.1, 0.41),
        inp("da", 0.0, 0.58e-6),
        inp("dth", 0.0, 0.029),
    ];
    (ast, inputs)
}

#[test]
fn gum_h1_end_gauge_published_values() {
    let (ast, inputs) = end_gauge();
    let r = analyze(&ast, &inputs).unwrap();
    let nm = 1e-9;
    // first order: printed 32 nm (H.6c)
    assert_eq!((r.first_order_variance.sqrt() / nm).round(), 32.0);
    // the two cross terms are the only nonzero Hessian entries: lS*u(da)*u(th), lS*u(aS)*u(dth)
    let t1 = 0.05 * 0.58e-6 * 0.41 / nm;
    let t2 = 0.05 * 1.2e-6 * 0.029 / nm;
    close(r.hessian[4][3].abs() * 0.58e-6 * 0.41 / nm, t1, 1e-12);
    // printed 11.7 nm is within 2% of the product of the printed rounded inputs (11.9 nm)
    assert!((t1 - 11.7).abs() / 11.7 < 0.02);
    assert!((t2 - 1.7).abs() < 0.05);
    // second-order term equals t1^2 + t2^2 (nm^2); u_c becomes 34 nm (printed)
    close(r.second_order_term / (nm * nm), t1 * t1 + t2 * t2, 1e-9);
    let uc = (r.first_order_variance + r.second_order_term).sqrt() / nm;
    assert_eq!(uc.round(), 34.0);
    // JCGM 101 Table 11: MCM u = 36 nm is above the 34 nm second-order GUF value, below 38
    assert!(uc < 36.5 && uc > 31.5);
}

#[test]
fn json_operation_analyze_ad_is_typed_limited() {
    let req = json!({
        "schema":"plurimath.evaluate.gum/1","method":"analyze_ad",
        "parameters":{
            "ast": bin("*", bin("^", var("x"), num("2")), var("y")),
            "inputs":[
                {"name":"x","estimate":3.0,"uncertainty":0.5},
                {"name":"y","estimate":2.0,"uncertainty":0.25}]
        }
    });
    let out: Value =
        serde_json::from_str(&plurimath_numeric::evaluate_json(&req.to_string())).unwrap();
    assert_eq!(out["ok"], true, "{out}");
    let r = &out["result"];
    assert_eq!(r["class"], "limited");
    assert_eq!(r["value"], 18.0);
    assert_eq!(r["gradient"], json!([12.0, 9.0]));
    assert_eq!(r["hessian"], json!([[4.0, 6.0], [6.0, 0.0]]));
    let u2 = [0.25_f64, 0.0625];
    let expected = 0.5 * (16.0 * u2[0] * u2[0] + 2.0 * 36.0 * u2[0] * u2[1]);
    assert_eq!(r["second_order_term"].as_f64().unwrap(), expected);
}
