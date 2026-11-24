#![cfg(feature = "gpu")]
use plurimath_gpu::eft::*;
use plurimath_gpu::guard::*;
use plurimath_gpu::repr::*;

const ALL: Capabilities = Capabilities {
    shader_f64: true,
    exact_fma: true,
};
const F32_ONLY: Capabilities = Capabilities {
    shader_f64: false,
    exact_fma: false,
};
const DF: Capabilities = Capabilities {
    shader_f64: false,
    exact_fma: true,
};

#[test]
fn two_sum_and_two_prod_are_exact() {
    let pairs = [
        (1.0e8f32, 3.1f32),
        (0.1, 0.2),
        (123456.78, 0.000123),
        (-7.5e3, 7.49e3),
    ];
    for (a, b) in pairs {
        let (s, e) = two_sum(a, b);
        assert_eq!(f64::from(s) + f64::from(e), f64::from(a) + f64::from(b));
        let (p, e) = two_prod(a, b);
        assert_eq!(f64::from(p) + f64::from(e), f64::from(a) * f64::from(b));
    }
}

#[test]
fn double_float_beats_f32() {
    let (a, b) = (std::f64::consts::PI, std::f64::consts::E);
    let (da, db) = (DoubleFloat::from_f64(a), DoubleFloat::from_f64(b));
    let m = da.mul_df(db).to_f64();
    let s = da.add_df(db).to_f64();
    assert!(((m - a * b) / (a * b)).abs() < 1e-12);
    assert!(((s - (a + b)) / (a + b)).abs() < 1e-12);
    let plain = f64::from(a as f32 * b as f32);
    assert!(((m - a * b) / (a * b)).abs() < ((plain - a * b) / (a * b)).abs());
}

#[test]
fn selects_cheapest_supported_repr() {
    assert_eq!(select(1e-5, 10, ALL).unwrap().repr, Repr::F32);
    assert_eq!(select(1e-10, 10, ALL).unwrap().repr, Repr::DoubleFloat);
    assert_eq!(select(1e-14, 10, ALL).unwrap().repr, Repr::NativeF64);
    assert_eq!(select(1e-10, 10, DF).unwrap().repr, Repr::DoubleFloat);
}

#[test]
fn refuses_when_accuracy_cannot_be_honoured() {
    match select(1e-14, 10, DF) {
        Err(Refusal::AccuracyUnattainable { best_bound, .. }) => assert!(best_bound > 1e-14),
        o => panic!("{o:?}"),
    }
    assert!(matches!(
        select(1e-10, 10, F32_ONLY),
        Err(Refusal::AccuracyUnattainable { .. })
    ));
    assert_eq!(select(0.0, 10, ALL), Err(Refusal::InvalidRequest));
    assert_eq!(select(f64::NAN, 10, ALL), Err(Refusal::InvalidRequest));
    assert_eq!(select(1e-3, 0, ALL), Err(Refusal::InvalidRequest));
    assert!(matches!(
        select(0.5, 1 << 30, F32_ONLY),
        Err(Refusal::BoundVacuous { .. })
    ));
}

#[test]
fn bound_holds_for_f32_dot_product() {
    let n = 1000u64;
    let xs: Vec<f32> = (0..n).map(|i| 1.0 + (i as f32) * 0.001).collect();
    let ys: Vec<f32> = (0..n).map(|i| 2.0 - (i as f32) * 0.0007).collect();
    let dev = xs.iter().zip(&ys).fold(0f32, |acc, (x, y)| acc + x * y);
    let cpu: f64 = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let abs: f64 = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| (f64::from(*x) * f64::from(*y)).abs())
        .sum();
    let sel = select(1e-3, n, F32_ONLY).unwrap();
    assert_eq!(sel.repr, Repr::F32);
    assert!(within_tolerance(cpu, f64::from(dev), abs, sel));
    assert!(!within_tolerance(cpu, f64::from(dev) * 1.01, abs, sel));
}

#[test]
fn guards_catch_denormal_overflow_nonfinite() {
    assert_eq!(check_f32(0.0), Ok(0.0));
    assert_eq!(check_f32(1.5), Ok(1.5));
    assert_eq!(check_f32(1.0e-40), Err(GuardFailure::Denormal));
    assert_eq!(check_f32(-1.0e-40), Err(GuardFailure::Denormal));
    assert_eq!(check_f32(f32::MAX), Err(GuardFailure::Overflow));
    assert_eq!(check_f32(f32::INFINITY), Err(GuardFailure::NotFinite));
    assert_eq!(check_f32(f32::NAN), Err(GuardFailure::NotFinite));
    assert_eq!(
        check_all(&[1.0, 2.0, 1e-41, f32::NAN]),
        Err((2, GuardFailure::Denormal))
    );
    assert_eq!(check_all(&[1.0, 2.0]), Ok(()));
    // a product of two small normals underflows to a denormal: caught
    let p = 1.0e-20f32 * 1.0e-20f32;
    assert!(check_f32(p).is_err() || p == 0.0);
}

#[test]
fn escalation_never_silent() {
    assert_eq!(
        escalate(GuardFailure::Denormal, Repr::F32, ALL),
        Escalation::Bump(Repr::NativeF64)
    );
    // double-float shares f32's exponent range: no cure without native f64
    assert_eq!(
        escalate(GuardFailure::Denormal, Repr::DoubleFloat, DF),
        Escalation::NumericalFailure(GuardFailure::Denormal)
    );
    assert_eq!(
        escalate(GuardFailure::Overflow, Repr::NativeF64, ALL),
        Escalation::NumericalFailure(GuardFailure::Overflow)
    );
    assert_eq!(
        escalate(GuardFailure::NotFinite, Repr::F32, ALL),
        Escalation::NumericalFailure(GuardFailure::NotFinite)
    );
}
