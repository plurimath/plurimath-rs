//! Scalar abstraction for the GUM models (issue #16, first slice).
//!
//! Only one operation is shared so far: multiplication of two coefficients.
//! Callers are generic over `S: Scalar`, so dispatch is static and each
//! instance is monomorphized. `Q` is the legacy instance and keeps the exact
//! arithmetic that previously lived inline in `Taylor::multiply`.
use super::Q;

pub(crate) trait Scalar {
    fn mul(&self, rhs: &Self) -> Self;
}

impl Scalar for Q {
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
}

impl Scalar for f64 {
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let a = Q::new(1.into(), 3.into());
        let b = Q::new(3.into(), 7.into());
        assert_eq!(product(&a, &b), Q::new(1.into(), 7.into()));
    }
}
