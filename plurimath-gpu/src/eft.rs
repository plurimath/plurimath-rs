//! Error-free transformations and double-float (f32 pair) arithmetic.
//! These are the portable building blocks used when the adapter has no
//! native f64. Uses fused multiply-add for the exact product error.

/// Exact sum: `a + b == s + e` with `s = fl(a + b)` (Knuth two-sum).
pub fn two_sum(a: f32, b: f32) -> (f32, f32) {
    let s = a + b;
    let bb = s - a;
    let e = (a - (s - bb)) + (b - bb);
    (s, e)
}

/// Exact product: `a * b == p + e` with `p = fl(a * b)`, via FMA.
pub fn two_prod(a: f32, b: f32) -> (f32, f32) {
    let p = a * b;
    (p, a.mul_add(b, -p))
}

/// Unevaluated sum `hi + lo` of two f32 with `|lo| <= ulp(hi)/2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoubleFloat {
    pub hi: f32,
    pub lo: f32,
}

impl DoubleFloat {
    pub fn from_f32(x: f32) -> Self {
        Self { hi: x, lo: 0.0 }
    }

    /// Split an f64 into a double-float (rounds beyond ~48 bits).
    pub fn from_f64(x: f64) -> Self {
        let hi = x as f32;
        let lo = (x - f64::from(hi)) as f32;
        Self { hi, lo }
    }

    pub fn to_f64(self) -> f64 {
        f64::from(self.hi) + f64::from(self.lo)
    }

    pub fn add_df(self, o: Self) -> Self {
        let (s, e) = two_sum(self.hi, o.hi);
        let (t, f) = two_sum(self.lo, o.lo);
        let (s, e) = quick(s, e + t);
        let (s, e) = quick(s, e + f);
        Self { hi: s, lo: e }
    }

    pub fn mul_df(self, o: Self) -> Self {
        let (p, e) = two_prod(self.hi, o.hi);
        let e = e + (self.hi * o.lo + self.lo * o.hi);
        let (hi, lo) = quick(p, e);
        Self { hi, lo }
    }
}

fn quick(a: f32, b: f32) -> (f32, f32) {
    let s = a + b;
    (s, b - (s - a))
}
