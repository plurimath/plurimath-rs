//! Scalar abstraction for the GUM models (issue #16).
//!
//! Callers are generic over `S: Scalar`, so dispatch is static and each
//! instance is monomorphized. `Q` is the legacy instance and keeps the exact
//! arithmetic that previously lived inline in the GUM modules. Operations
//! that cannot stay exact (`sqrt`, `exp`, `ln`, `quantize`) return the
//! instance's `Wide` type: `N` for `Q`, `f64` for `f64`.
use super::{decimal_math, guard_sqrt, roundq, E, N, Q};
use num_traits::Zero;

pub(crate) trait Scalar: Sized + Clone {
    type Wide;
    fn s_zero() -> Self;
    fn s_one() -> Self;
    fn s_is_zero(&self) -> bool;
    fn from_i32(n: i32) -> Self;
    /// Convert an exact rational input into this instance.
    fn from_q(q: &Q) -> Self;
    /// Size of the value in bits, used to bound exact growth. Fixed-width
    /// instances report 0 because they cannot grow.
    fn bit_size(&self) -> u64;
    /// The value as an `i32` when it is exactly an integer in range.
    fn to_i32_exact(&self) -> Option<i32>;
    fn negate(&self) -> Self {
        Self::s_zero().sub(self)
    }
    fn add(&self, rhs: &Self) -> Self;
    fn sub(&self, rhs: &Self) -> Self;
    fn mul(&self, rhs: &Self) -> Self;
    /// `None` when the divisor is zero.
    fn div(&self, rhs: &Self) -> Option<Self>;
    fn sqrt(&self, digits: u32) -> Result<Self::Wide, E>;
    #[allow(dead_code)] // no model calls exp/ln yet; covered by unit tests
    fn exp(&self, digits: u32) -> Result<Self::Wide, E>;
    #[allow(dead_code)]
    fn ln(&self, digits: u32) -> Result<Self::Wide, E>;
    /// Round to `digits` significant digits.
    fn quantize(&self, digits: u32) -> Self::Wide;
}

#[allow(dead_code)]
fn q_guard(op: &str, q: &Q, digits: u32) -> Result<N, E> {
    decimal_math::guard(op, &N::Exact(q.clone(), false), digits)
}

impl Scalar for Q {
    type Wide = N;
    fn s_zero() -> Self {
        Zero::zero()
    }
    fn s_one() -> Self {
        num_traits::One::one()
    }
    fn s_is_zero(&self) -> bool {
        Zero::is_zero(self)
    }
    fn from_i32(n: i32) -> Self {
        Q::from_integer(n.into())
    }
    fn from_q(q: &Q) -> Self {
        q.clone()
    }
    fn bit_size(&self) -> u64 {
        self.numer().bits().max(self.denom().bits())
    }
    fn to_i32_exact(&self) -> Option<i32> {
        use num_traits::ToPrimitive;
        self.is_integer()
            .then(|| self.to_integer().to_i32())
            .flatten()
    }
    fn add(&self, rhs: &Self) -> Self {
        self + rhs
    }
    fn sub(&self, rhs: &Self) -> Self {
        self - rhs
    }
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
    fn div(&self, rhs: &Self) -> Option<Self> {
        (!rhs.is_zero()).then(|| self / rhs)
    }
    fn sqrt(&self, digits: u32) -> Result<N, E> {
        guard_sqrt(self, digits)
    }
    fn exp(&self, digits: u32) -> Result<N, E> {
        q_guard("exp", self, digits)
    }
    fn ln(&self, digits: u32) -> Result<N, E> {
        q_guard("log", self, digits)
    }
    fn quantize(&self, digits: u32) -> N {
        roundq(self, digits)
    }
}

fn finite(x: f64) -> Result<f64, E> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err("NonFiniteResultError")
    }
}

impl Scalar for f64 {
    type Wide = f64;
    fn s_zero() -> Self {
        0.0
    }
    fn s_one() -> Self {
        1.0
    }
    fn s_is_zero(&self) -> bool {
        *self == 0.0
    }
    fn from_i32(n: i32) -> Self {
        f64::from(n)
    }
    fn from_q(q: &Q) -> Self {
        num_traits::ToPrimitive::to_f64(q).unwrap_or(f64::NAN)
    }
    fn bit_size(&self) -> u64 {
        0
    }
    fn to_i32_exact(&self) -> Option<i32> {
        (self.fract() == 0.0 && self.abs() <= f64::from(i32::MAX)).then_some(*self as i32)
    }
    fn add(&self, rhs: &Self) -> Self {
        self + rhs
    }
    fn sub(&self, rhs: &Self) -> Self {
        self - rhs
    }
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
    fn div(&self, rhs: &Self) -> Option<Self> {
        (*rhs != 0.0).then(|| self / rhs)
    }
    fn sqrt(&self, _digits: u32) -> Result<f64, E> {
        if *self < 0.0 {
            return Err("NonFiniteResultError");
        }
        finite(libm::sqrt(*self))
    }
    fn exp(&self, _digits: u32) -> Result<f64, E> {
        finite(libm::exp(*self))
    }
    fn ln(&self, _digits: u32) -> Result<f64, E> {
        if *self < 0.0 {
            return Err("MathDomainError");
        }
        finite(libm::log(*self))
    }
    fn quantize(&self, digits: u32) -> f64 {
        if !self.is_finite() || *self == 0.0 {
            return *self;
        }
        format!("{:.*e}", digits.saturating_sub(1) as usize, self)
            .parse()
            .unwrap_or(*self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i64, d: i64) -> Q {
        Q::new(n.into(), d.into())
    }

    fn product<S: Scalar>(a: &S, b: &S) -> S {
        a.mul(b)
    }

    #[test]
    fn f64_matches_native_product_bits() {
        for (a, b) in [(0.1_f64, 0.2), (-3.5, 1e300), (1e-320, 0.5), (0.0, -0.0)] {
            assert_eq!(product(&a, &b).to_bits(), (a * b).to_bits());
        }
    }

    #[test]
    fn q_matches_exact_product() {
        assert_eq!(product(&q(1, 3), &q(3, 7)), q(1, 7));
    }

    #[test]
    fn q_add_sub_div_exact() {
        let (a, b) = (q(1, 3), q(1, 6));
        assert_eq!(Scalar::add(&a, &b), q(1, 2));
        assert_eq!(Scalar::sub(&a, &b), q(1, 6));
        assert_eq!(Scalar::div(&a, &b), Some(q(2, 1)));
        assert_eq!(Scalar::div(&a, &Q::zero()), None);
    }

    #[test]
    fn f64_add_sub_div_bits() {
        let (a, b) = (0.1_f64, 0.7_f64);
        assert_eq!(Scalar::add(&a, &b).to_bits(), (a + b).to_bits());
        assert_eq!(Scalar::sub(&a, &b).to_bits(), (a - b).to_bits());
        assert_eq!(Scalar::div(&a, &b).unwrap().to_bits(), (a / b).to_bits());
        assert_eq!(Scalar::div(&a, &0.0), None);
    }

    #[test]
    fn q_transcendentals_match_legacy_helpers() {
        let x = q(2, 1);
        assert_eq!(
            Scalar::sqrt(&x, 20).unwrap().wire(),
            guard_sqrt(&x, 20).unwrap().wire()
        );
        let e = Scalar::exp(&q(1, 1), 20).unwrap().wire();
        assert!(e.to_string().contains("0.27182818284590452354e1"));
        let l = Scalar::ln(&q(2, 1), 20).unwrap().wire();
        assert!(l.to_string().contains("0.69314718055994530942e0"));
        assert_eq!(
            Scalar::quantize(&q(1, 3), 4).wire(),
            roundq(&q(1, 3), 4).wire()
        );
        assert!(Scalar::sqrt(&q(-1, 1), 10).is_err());
    }

    #[test]
    fn f64_transcendentals() {
        assert_eq!(
            Scalar::sqrt(&2.0_f64, 0).unwrap().to_bits(),
            2.0_f64.sqrt().to_bits()
        );
        assert!(Scalar::sqrt(&-1.0_f64, 0).is_err());
        assert_eq!(
            Scalar::exp(&1.0_f64, 0).unwrap().to_bits(),
            libm::exp(1.0).to_bits()
        );
        assert!(Scalar::exp(&1000.0_f64, 0).is_err());
        assert_eq!(Scalar::ln(&1.0_f64, 0).unwrap(), 0.0);
        assert_eq!(Scalar::ln(&-1.0_f64, 0), Err("MathDomainError"));
        assert_eq!(Scalar::quantize(&123456.0_f64, 3), 123000.0);
    }
}
