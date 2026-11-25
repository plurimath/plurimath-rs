//! Numerical guards: GPUs flush denormals to zero and may saturate on
//! overflow, so such values must escalate, never pass silently.

use crate::repr::Repr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardFailure {
    NotFinite,
    Overflow,
    /// Non-zero but below the smallest normal: flushed on device.
    Denormal,
}

/// What the caller should do about a guarded value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escalation {
    /// Retry with a wider representation.
    Bump(Repr),
    /// No wider representation is available: report NumericalFailure.
    NumericalFailure(GuardFailure),
}

/// Fraction of f32::MAX above which a value is treated as near overflow.
const OVERFLOW_MARGIN: f32 = 0.5;

/// Check an f32 value. Exact zero is fine; denormals and near-overflow
/// values are failures. A result within a factor 2^24 of the normal
/// range floor is also refused, since a relative bound is meaningless
/// once the ulp itself is denormal.
pub fn check_f32(x: f32) -> Result<f32, GuardFailure> {
    if !x.is_finite() {
        return Err(GuardFailure::NotFinite);
    }
    let a = x.abs();
    if a > f32::MAX * OVERFLOW_MARGIN {
        return Err(GuardFailure::Overflow);
    }
    if a != 0.0 && a < f32::MIN_POSITIVE {
        return Err(GuardFailure::Denormal);
    }
    Ok(x)
}

/// Check a slice, reporting the first failing index.
pub fn check_all(xs: &[f32]) -> Result<(), (usize, GuardFailure)> {
    xs.iter()
        .enumerate()
        .try_for_each(|(i, &x)| check_f32(x).map(|_| ()).map_err(|e| (i, e)))
}

/// Decide the escalation for a failed guard given the current repr and
/// what the adapter offers. Double-float shares f32's exponent range, so
/// it does not cure denormal/overflow; only native f64 does.
pub fn escalate(
    failure: GuardFailure,
    current: Repr,
    caps: crate::repr::Capabilities,
) -> Escalation {
    if current != Repr::NativeF64 && caps.shader_f64 && failure != GuardFailure::NotFinite {
        Escalation::Bump(Repr::NativeF64)
    } else {
        Escalation::NumericalFailure(failure)
    }
}
