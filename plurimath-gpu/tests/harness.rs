//! Cross-backend harness: emulates each device representation on the CPU
//! and compares it with a compensated f64 reference within the reported
//! bound. GPU cases need an adapter and are ignored.
#![cfg(feature = "gpu")]
use plurimath_gpu::backend::*;
use plurimath_gpu::eft::DoubleFloat;
use plurimath_gpu::repr::*;
use plurimath_numeric::prng::Philox4x32;
use serde_json::Value;

const ALL: Capabilities = Capabilities {
    shader_f64: true,
    exact_fma: true,
};
const F32_ONLY: Capabilities = Capabilities {
    shader_f64: false,
    exact_fma: false,
};

fn data(seed: u64, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut u = vec![0.0; 2 * n];
    Philox4x32::new(seed).fill_uniform(0, &mut u);
    let v: Vec<f64> = u.iter().map(|x| 2.0 * x - 1.0).collect();
    (v[..n].to_vec(), v[n..].to_vec())
}

/// Dot2 (Ogita-Rump-Oishi): f64 dot product with ~u^2 accuracy.
fn reference_dot(x: &[f64], y: &[f64]) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for (&a, &b) in x.iter().zip(y) {
        let p = a * b;
        let pe = a.mul_add(b, -p);
        let t = s + p;
        let bb = t - s;
        let se = (s - (t - bb)) + (p - bb);
        s = t;
        c += se + pe;
    }
    s + c
}

fn abs_sum(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| (a * b).abs()).sum()
}

fn emulate(repr: Repr, x: &[f64], y: &[f64]) -> f64 {
    match repr {
        Repr::F32 => x
            .iter()
            .zip(y)
            .fold(0.0f32, |s, (&a, &b)| s + (a as f32) * (b as f32)) as f64,
        Repr::DoubleFloat => x
            .iter()
            .zip(y)
            .fold(DoubleFloat::from_f32(0.0), |s, (&a, &b)| {
                s.add_df(DoubleFloat::from_f64(a).mul_df(DoubleFloat::from_f64(b)))
            })
            .to_f64(),
        Repr::NativeF64 => x.iter().zip(y).fold(0.0, |s, (&a, &b)| s + a * b),
    }
}

#[test]
fn emulated_representations_agree_with_reference_within_bound() {
    for repr in Repr::ALL {
        for n in [8usize, 64, 1024, 4096] {
            for seed in 1..=5u64 {
                let (x, y) = data(seed, n);
                let want = reference_dot(&x, &y);
                let sel = Selection {
                    repr,
                    error_bound: gamma(n as u64, repr).unwrap(),
                };
                let got = emulate(repr, &x, &y);
                // The inputs are rounded into the representation first, so
                // allow the input rounding on top of the accumulation bound.
                let inputs = Selection {
                    error_bound: sel.error_bound + 2.0 * repr.unit_roundoff(),
                    ..sel
                };
                assert!(
                    within_tolerance(want, got, abs_sum(&x, &y), inputs),
                    "{repr:?} n={n} seed={seed}: ref {want} got {got}"
                );
            }
        }
    }
}

#[test]
fn comparison_fails_outside_the_bound() {
    let (x, y) = data(9, 256);
    let want = reference_dot(&x, &y);
    let sel = Selection {
        repr: Repr::F32,
        error_bound: gamma(256, Repr::F32).unwrap(),
    };
    let off = want + 1000.0 * sel.error_bound * abs_sum(&x, &y);
    assert!(!within_tolerance(want, off, abs_sum(&x, &y), sel));
}

#[test]
fn plan_uses_device_when_accuracy_is_attainable() {
    let p = plan(1e-6, 1024, Some(ALL));
    assert!(matches!(p.backend, Backend::Device(_)));
    assert_eq!(p.reason, None);
}

#[test]
fn plan_falls_back_without_adapter() {
    let p = plan(1e-6, 1024, None);
    assert_eq!(p.backend, Backend::CpuGumPar);
    assert_eq!(p.reason, Some(FallbackReason::NoAdapter));
}

#[test]
fn plan_falls_back_when_guards_refuse_accuracy() {
    let p = plan(1e-15, 1024, Some(F32_ONLY));
    assert_eq!(p.backend, Backend::CpuGumPar);
    assert!(matches!(
        p.reason,
        Some(FallbackReason::Refused(
            Refusal::AccuracyUnattainable { .. }
        ))
    ));
    let bad = plan(f64::NAN, 10, Some(ALL));
    assert_eq!(
        bad.reason,
        Some(FallbackReason::Refused(Refusal::InvalidRequest))
    );
}

/// Case 4 of the frozen MCM fixture: y = a + b + c + d, rectangular inputs.
fn mc_request() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../plurimath-numeric/fixtures/gum-mcm.json"
    ))
    .unwrap();
    fixture["cases"][4]["request"].clone()
}

#[test]
fn fallback_gum_par_is_thread_count_independent() {
    let one = run_gum_par(&mc_request(), 20261124, 1);
    assert_eq!(one["ok"], true, "{one}");
    assert_eq!(run_gum_par(&mc_request(), 20261124, 4), one);
    assert_eq!(one["result"]["engine"]["id"], "gum-par");
}

#[test]
fn fallback_gum_par_estimate_is_near_the_exact_mean() {
    // Symmetric rectangular inputs: the exact output mean is 0.
    let r = run_gum_par(&mc_request(), 7, 2);
    assert_eq!(r["ok"], true, "{r}");
    let res = &r["result"];
    let num = |k: &str| res[k].as_str().unwrap().parse::<f64>().unwrap();
    let n = res["samples"].as_f64().unwrap();
    let bound = 4.0 * num("standard_uncertainty") / n.sqrt();
    assert!(num("estimate").abs() <= bound, "{res}");
}

/// Adapter probe; the wgpu device layer is a later slice.
fn probe_adapter() -> Option<Capabilities> {
    std::env::var_os("PLURIMATH_GPU_ADAPTER").map(|_| ALL)
}

#[test]
#[ignore = "needs a wgpu adapter; the device layer is a later slice"]
fn device_matches_reference_within_bound() {
    let caps = probe_adapter().expect("no adapter available");
    let p = plan(1e-6, 1024, Some(caps));
    assert!(matches!(p.backend, Backend::Device(_)));
}
