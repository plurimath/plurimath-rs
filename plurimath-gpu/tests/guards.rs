//! Denormal and overflow guard behaviour at its boundaries (#20: guards
//! proven by tests). Device flush-to-zero is emulated by the guard itself;
//! no adapter is needed.
#![cfg(feature = "gpu")]
use plurimath_gpu::guard::*;
use plurimath_gpu::repr::*;

const REPRS: [Repr; 3] = [Repr::F32, Repr::DoubleFloat, Repr::NativeF64];
const FAILURES: [GuardFailure; 3] = [
    GuardFailure::NotFinite,
    GuardFailure::Overflow,
    GuardFailure::Denormal,
];

fn caps(shader_f64: bool) -> Capabilities {
    Capabilities {
        shader_f64,
        exact_fma: true,
    }
}

#[test]
fn denormal_boundary_is_the_smallest_normal() {
    assert_eq!(check_f32(f32::MIN_POSITIVE), Ok(f32::MIN_POSITIVE));
    assert_eq!(check_f32(-f32::MIN_POSITIVE), Ok(-f32::MIN_POSITIVE));
    let largest_denormal = f32::from_bits(f32::MIN_POSITIVE.to_bits() - 1);
    let smallest_denormal = f32::from_bits(1);
    for x in [largest_denormal, smallest_denormal] {
        assert!(x != 0.0 && x < f32::MIN_POSITIVE);
        assert_eq!(check_f32(x), Err(GuardFailure::Denormal));
        assert_eq!(check_f32(-x), Err(GuardFailure::Denormal));
    }
}

#[test]
fn zero_of_either_sign_is_accepted() {
    assert_eq!(check_f32(0.0), Ok(0.0));
    assert!(check_f32(-0.0).is_ok());
}

#[test]
fn overflow_boundary_is_half_of_f32_max() {
    let limit = f32::MAX * 0.5;
    assert_eq!(check_f32(limit), Ok(limit));
    assert_eq!(check_f32(-limit), Ok(-limit));
    let above = f32::from_bits(limit.to_bits() + 1);
    assert_eq!(check_f32(above), Err(GuardFailure::Overflow));
    assert_eq!(check_f32(-above), Err(GuardFailure::Overflow));
    assert_eq!(check_f32(f32::MAX), Err(GuardFailure::Overflow));
    assert_eq!(check_f32(f32::MIN), Err(GuardFailure::Overflow));
}

#[test]
fn non_finite_is_reported_as_such_for_both_signs() {
    for x in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -f32::NAN] {
        assert_eq!(check_f32(x), Err(GuardFailure::NotFinite));
    }
}

#[test]
fn arithmetic_that_leaves_the_safe_range_is_caught() {
    // product underflows into the denormal range
    assert_eq!(
        check_f32(1.0e-20f32 * 3.0e-20f32),
        Err(GuardFailure::Denormal)
    );
    // quotient underflows into the denormal range
    assert_eq!(
        check_f32(1.0e-30f32 / 1.0e10f32),
        Err(GuardFailure::Denormal)
    );
    // sum of near-maximal values saturates to infinity
    assert_eq!(
        check_f32(f32::MAX * 0.75 + f32::MAX * 0.75),
        Err(GuardFailure::NotFinite)
    );
    // product that stays finite but past the margin
    assert_eq!(
        check_f32(3.0e19f32 * 1.0e19f32),
        Err(GuardFailure::Overflow)
    );
    // ordinary values pass untouched
    assert_eq!(
        check_f32(3.0e10f32 * 1.0e-10f32),
        Ok(3.0e10f32 * 1.0e-10f32)
    );
}

#[test]
fn check_all_reports_the_first_failing_index_regardless_of_kind() {
    assert_eq!(check_all(&[]), Ok(()));
    assert_eq!(check_all(&[0.0, 1.0, -2.5, f32::MIN_POSITIVE]), Ok(()));
    assert_eq!(
        check_all(&[1e-41, f32::NAN]),
        Err((0, GuardFailure::Denormal))
    );
    assert_eq!(
        check_all(&[1.0, f32::MAX, 1e-41]),
        Err((1, GuardFailure::Overflow))
    );
    assert_eq!(
        check_all(&[1.0, 2.0, f32::INFINITY, f32::MAX]),
        Err((2, GuardFailure::NotFinite))
    );
}

#[test]
fn escalation_matrix_is_never_silent() {
    for failure in FAILURES {
        for current in REPRS {
            for shader_f64 in [false, true] {
                let outcome = escalate(failure, current, caps(shader_f64));
                let can_bump =
                    current != Repr::NativeF64 && shader_f64 && failure != GuardFailure::NotFinite;
                if can_bump {
                    assert_eq!(outcome, Escalation::Bump(Repr::NativeF64));
                } else {
                    // no wider representation helps: surface the failure itself
                    assert_eq!(outcome, Escalation::NumericalFailure(failure));
                }
            }
        }
    }
}

#[test]
fn double_float_and_f32_only_adapters_cannot_cure_denormal_or_overflow() {
    for failure in [GuardFailure::Denormal, GuardFailure::Overflow] {
        for current in [Repr::F32, Repr::DoubleFloat] {
            assert_eq!(
                escalate(failure, current, caps(false)),
                Escalation::NumericalFailure(failure)
            );
        }
    }
}
