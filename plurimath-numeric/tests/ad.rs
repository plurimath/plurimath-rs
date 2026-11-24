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
    close(r.hessian_diagonal[0], 2.0 * y - x.sin() / y, 1e-14);
    close(r.hessian_diagonal[1], 2.0 * x.sin() / (y * y * y), 1e-14);
    let expected = 0.5 * (r.hessian_diagonal[0] * 0.01 + r.hessian_diagonal[1] * 0.04);
    close(r.second_order_term, expected, 1e-15);
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

/// Product model Y = X1 X2 (GUM H.1.7-style): f_ii = 0, so the correction is 0,
/// and with f = x^3 the term is 3 x u^2 exactly.
#[test]
fn second_order_term_closed_forms() {
    let r = analyze(
        &bin("*", var("a"), var("b")),
        &[inp("a", 2.0, 0.5), inp("b", 3.0, 0.5)],
    )
    .unwrap();
    assert_eq!(r.hessian_diagonal, vec![0.0, 0.0]);
    assert_eq!(r.second_order_term, 0.0);
    let cube = analyze(&bin("^", var("x"), num("3")), &[inp("x", 2.0, 0.1)]).unwrap();
    close(cube.second_order_term, 3.0 * 2.0 * 0.01, 1e-15);
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
        r.hessian_diagonal[0],
        x.exp() * (x.ln() + 2.0 / x - 1.0 / (x * x)),
        1e-14,
    );
    let s = analyze(&bin("^", var("x"), num("0.5")), &[inp("x", 4.0, 1.0)]).unwrap();
    close(s.gradient[0], 0.25, 1e-14);
    close(s.hessian_diagonal[0], -1.0 / 32.0, 1e-14);
    let err = analyze(&bin("/", num("1"), var("x")), &[inp("x", 0.0, 1.0)]);
    assert_eq!(err.unwrap_err(), "DivisionByZeroError");
    assert!(analyze(&call("tan", var("x")), &[inp("x", 1.0, 1.0)]).is_err());
}
