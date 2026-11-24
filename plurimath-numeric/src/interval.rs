//! Interval arithmetic with outward rounding and a rigorous LPU-adequacy
//! verdict (issue #18).
//!
//! Every operation returns an interval that contains the exact real result
//! for every pair of operands drawn from the input intervals. Rust has no
//! directed rounding mode, so each correctly rounded `f64` result is widened
//! by one ulp in each direction (`sqrt` and `+ - * /` are correctly rounded
//! under IEEE 754, so the true value lies within one ulp). `exp` and `ln` go
//! through `libm`, which is documented to be below 1 ulp, so they are widened
//! by two ulps. The enclosure is therefore a proof, not a sample.
//!
//! The verdict uses the mean-value form around the input estimates `c`:
//! over a box `X`, `f(x) - lin(x)` lies in `sum_i (G_i - f_i(c)) (X_i - c_i)`
//! where `G_i` encloses `df/dx_i` over `X` (taken from forward-mode interval
//! duals, the same gradient machinery as #17). Where that over-widens
//! (dependency problem) the box is bisected within a recorded budget.
use crate::scalar::Scalar;
use crate::E;
use serde_json::{json, Value as J};

fn up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f64::from_bits(1);
    }
    let b = x.to_bits();
    f64::from_bits(if x > 0.0 { b + 1 } else { b - 1 })
}

fn down(x: f64) -> f64 {
    -up(-x)
}

fn up_n(mut x: f64, n: u32) -> f64 {
    for _ in 0..n {
        x = up(x);
    }
    x
}

fn down_n(x: f64, n: u32) -> f64 {
    -up_n(-x, n)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Self {
        assert!(lo <= hi, "empty interval");
        Interval { lo, hi }
    }
    pub fn point(x: f64) -> Self {
        Interval { lo: x, hi: x }
    }
    pub fn width(&self) -> f64 {
        up(self.hi - self.lo)
    }
    pub fn mid(&self) -> f64 {
        self.lo / 2.0 + self.hi / 2.0
    }
    /// Largest absolute value in the interval (upper bound).
    pub fn mag(&self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }
    /// Smallest absolute value in the interval (lower bound).
    pub fn mig(&self) -> f64 {
        if self.lo <= 0.0 && self.hi >= 0.0 {
            0.0
        } else {
            self.lo.abs().min(self.hi.abs())
        }
    }
    pub fn contains(&self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }
    fn widen(lo: f64, hi: f64, n: u32) -> Self {
        Interval {
            lo: down_n(lo, n),
            hi: up_n(hi, n),
        }
    }
    fn is_finite(&self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }
}

fn fin(i: Interval) -> Result<Interval, E> {
    if i.is_finite() {
        Ok(i)
    } else {
        Err("NonFiniteResultError")
    }
}

impl Scalar for Interval {
    type Wide = Interval;
    fn add(&self, r: &Self) -> Self {
        Interval::widen(self.lo + r.lo, self.hi + r.hi, 1)
    }
    fn sub(&self, r: &Self) -> Self {
        Interval::widen(self.lo - r.hi, self.hi - r.lo, 1)
    }
    fn mul(&self, r: &Self) -> Self {
        let p = [
            self.lo * r.lo,
            self.lo * r.hi,
            self.hi * r.lo,
            self.hi * r.hi,
        ];
        let lo = p.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = p.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Interval::widen(lo, hi, 1)
    }
    /// `None` when the divisor interval contains zero.
    fn div(&self, r: &Self) -> Option<Self> {
        if r.lo <= 0.0 && r.hi >= 0.0 {
            return None;
        }
        let p = [
            self.lo / r.lo,
            self.lo / r.hi,
            self.hi / r.lo,
            self.hi / r.hi,
        ];
        let lo = p.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = p.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some(Interval::widen(lo, hi, 1))
    }
    fn sqrt(&self, _digits: u32) -> Result<Self, E> {
        if self.lo < 0.0 {
            return Err("NonFiniteResultError");
        }
        fin(Interval::widen(libm::sqrt(self.lo), libm::sqrt(self.hi), 1)).map(|i| Interval {
            lo: i.lo.max(0.0),
            hi: i.hi,
        })
    }
    fn exp(&self, _digits: u32) -> Result<Self, E> {
        fin(Interval::widen(libm::exp(self.lo), libm::exp(self.hi), 2))
    }
    fn ln(&self, _digits: u32) -> Result<Self, E> {
        if self.lo <= 0.0 {
            return Err("MathDomainError");
        }
        fin(Interval::widen(libm::log(self.lo), libm::log(self.hi), 2))
    }
    /// Intervals are never rounded to a decimal grid; the enclosure is kept.
    fn quantize(&self, _digits: u32) -> Self {
        *self
    }
}

/// Interval value with an interval first derivative along one seeded input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IDual {
    pub v: Interval,
    pub d: Interval,
}

impl IDual {
    pub fn constant(c: f64) -> Self {
        IDual {
            v: Interval::point(c),
            d: Interval::point(0.0),
        }
    }
    pub fn variable(x: Interval, seed: bool) -> Self {
        IDual {
            v: x,
            d: Interval::point(if seed { 1.0 } else { 0.0 }),
        }
    }
}

impl Scalar for IDual {
    type Wide = IDual;
    fn add(&self, r: &Self) -> Self {
        IDual {
            v: self.v.add(&r.v),
            d: self.d.add(&r.d),
        }
    }
    fn sub(&self, r: &Self) -> Self {
        IDual {
            v: self.v.sub(&r.v),
            d: self.d.sub(&r.d),
        }
    }
    fn mul(&self, r: &Self) -> Self {
        IDual {
            v: self.v.mul(&r.v),
            d: self.d.mul(&r.v).add(&self.v.mul(&r.d)),
        }
    }
    fn div(&self, r: &Self) -> Option<Self> {
        let v = self.v.div(&r.v)?;
        // (a/b)' = (a' - (a/b) b') / b
        let d = self.d.sub(&v.mul(&r.d)).div(&r.v)?;
        Some(IDual { v, d })
    }
    fn sqrt(&self, digits: u32) -> Result<Self, E> {
        let v = self.v.sqrt(digits)?;
        let two_v = v.add(&v);
        let d = self.d.div(&two_v).ok_or("NonFiniteResultError")?;
        Ok(IDual { v, d })
    }
    fn exp(&self, digits: u32) -> Result<Self, E> {
        let v = self.v.exp(digits)?;
        Ok(IDual {
            v,
            d: v.mul(&self.d),
        })
    }
    fn ln(&self, digits: u32) -> Result<Self, E> {
        let v = self.v.ln(digits)?;
        let d = self.d.div(&self.v).ok_or("NonFiniteResultError")?;
        Ok(IDual { v, d })
    }
    fn quantize(&self, _digits: u32) -> Self {
        *self
    }
}

/// Verdict of the rigorous adequacy test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Proven: the linearization error is within the threshold everywhere in
    /// the coverage box.
    Adequate,
    /// Proven: some point of the box has a linearization error above the
    /// threshold.
    Inadequate,
    /// The budget ran out before either proof closed.
    Inconclusive,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Adequate => "adequate",
            Verdict::Inadequate => "inadequate",
            Verdict::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Adequacy {
    pub verdict: Verdict,
    /// Enclosure of `f(x) - lin(x)` over the whole coverage box.
    pub error: Interval,
    /// Upper bound of the combined standard uncertainty of `y` (LPU).
    pub u_c_hi: f64,
    /// Rigorous upper bound of `max |error| / u_c` over the box.
    pub ratio_hi: f64,
    pub threshold: f64,
    pub boxes_used: usize,
    pub budget: usize,
    pub max_depth: usize,
}

impl Adequacy {
    pub fn to_json(&self) -> J {
        json!({
            "verdict": self.verdict.as_str(),
            "error_enclosure": [self.error.lo, self.error.hi],
            "u_c_upper": self.u_c_hi,
            "ratio_upper": self.ratio_hi,
            "threshold": self.threshold,
            "subdivision": {"boxes_used": self.boxes_used, "budget": self.budget, "max_depth": self.max_depth},
        })
    }
}

type Model<'a> = &'a dyn Fn(&[IDual]) -> Result<IDual, E>;

fn eval_box(f: Model, bx: &[Interval]) -> Result<(Interval, Vec<Interval>), E> {
    let mut grad = Vec::with_capacity(bx.len());
    let mut val = None;
    for i in 0..bx.len() {
        let args: Vec<IDual> = bx
            .iter()
            .enumerate()
            .map(|(j, x)| IDual::variable(*x, i == j))
            .collect();
        let r = f(&args)?;
        grad.push(r.d);
        val = Some(r.v);
    }
    Ok((val.ok_or("EmptyModelError")?, grad))
}

/// Decide adequacy of the first-order LPU for `y = f(x)` with estimates `c`,
/// standard uncertainties `u` (independent inputs) and coverage box
/// `c_i +/- k u_i`. The linearization error is judged adequate when
/// `max |f - lin| <= threshold * u_c(y)`. The box is bisected (widest
/// relative side first) until every piece is decided or `budget` boxes are
/// used.
pub fn lpu_adequacy(
    f: Model,
    c: &[f64],
    u: &[f64],
    k: f64,
    threshold: f64,
    budget: usize,
) -> Result<Adequacy, E> {
    if c.len() != u.len()
        || c.is_empty()
        || k.is_nan()
        || threshold.is_nan()
        || k <= 0.0
        || threshold <= 0.0
    {
        return Err("ArgumentError");
    }
    let cpt: Vec<Interval> = c.iter().map(|x| Interval::point(*x)).collect();
    let (f_c, g_c) = eval_box(f, &cpt)?;
    // u_c^2 = sum (f_i u_i)^2, enclosed.
    let mut var = Interval::point(0.0);
    for (g, ui) in g_c.iter().zip(u) {
        let t = g.mul(&Interval::point(*ui));
        var = var.add(&t.mul(&t));
    }
    let u_c = var.sqrt(0)?;
    if u_c.lo <= 0.0 {
        return Err("DegenerateUncertaintyError");
    }
    let root: Vec<Interval> = c
        .iter()
        .zip(u)
        .map(|(ci, ui)| {
            let h = Interval::point(k).mul(&Interval::point(*ui));
            Interval::new(
                Interval::point(*ci).sub(&h).lo,
                Interval::point(*ci).add(&h).hi,
            )
        })
        .collect();
    let limit = threshold * u_c.lo;
    let mut stack = vec![(root, c.to_vec(), 0usize)];
    let mut error: Option<Interval> = None;
    let mut boxes = 0usize;
    let mut max_depth = 0usize;
    let mut worst = 0.0_f64;
    let mut verdict = Verdict::Adequate;
    while let Some((bx, mid, depth)) = stack.pop() {
        boxes += 1;
        max_depth = max_depth.max(depth);
        let midpt: Vec<Interval> = mid.iter().map(|x| Interval::point(*x)).collect();
        let (f_m, _) = eval_box(f, &midpt)?;
        let (_, g) = eval_box(f, &bx)?;
        // f(x) - lin(x) = [f(m) - f(c) - sum f_i(c)(m_i - c_i)]
        //               + sum (G_i - f_i(c)) (x_i - m_i),  m = box centre.
        // At the root the centre is `c` itself and the bracket is exactly 0.
        let mut err = if mid == c {
            Interval::point(0.0)
        } else {
            f_m.sub(&f_c)
        };
        for i in 0..bx.len() {
            let dm = Interval::point(mid[i]).sub(&Interval::point(c[i]));
            err = err.sub(&g_c[i].mul(&dm));
            let dg = g[i].sub(&g_c[i]);
            err = err.add(&dg.mul(&bx[i].sub(&Interval::point(mid[i]))));
        }
        error = Some(match error {
            None => err,
            Some(e) => Interval {
                lo: e.lo.min(err.lo),
                hi: e.hi.max(err.hi),
            },
        });
        let m = err.mag();
        if m <= limit {
            worst = worst.max(m);
            continue;
        }
        // Try to prove inadequacy at the box centre or a corner (point
        // evaluations are enclosed, so a lower bound on |error| is a proof).
        let mut proven = false;
        for pt in [
            mid.clone(),
            bx.iter().map(|i| i.lo).collect(),
            bx.iter().map(|i| i.hi).collect(),
        ] {
            let p: Vec<Interval> = pt.iter().map(|x| Interval::point(*x)).collect();
            let (fp, _) = eval_box(f, &p)?;
            let mut e = fp.sub(&f_c);
            for i in 0..pt.len() {
                let dx = Interval::point(pt[i]).sub(&Interval::point(c[i]));
                e = e.sub(&g_c[i].mul(&dx));
            }
            if e.mig() > threshold * u_c.hi {
                proven = true;
                break;
            }
        }
        if proven {
            worst = worst.max(m);
            verdict = Verdict::Inadequate;
            break;
        }
        if boxes + stack.len() + 2 > budget {
            worst = worst.max(m);
            verdict = Verdict::Inconclusive;
            continue;
        }
        let (mut idx, mut wmax) = (0, -1.0);
        for (i, side) in bx.iter().enumerate() {
            let w = side.width() / (k * u[i]).max(f64::MIN_POSITIVE);
            if w > wmax {
                wmax = w;
                idx = i;
            }
        }
        let (mut a, mut b) = (bx.clone(), bx.clone());
        let cut = bx[idx].mid();
        a[idx] = Interval::new(bx[idx].lo, cut);
        b[idx] = Interval::new(cut, bx[idx].hi);
        let (mut ma, mut mb) = (mid.clone(), mid.clone());
        ma[idx] = a[idx].mid();
        mb[idx] = b[idx].mid();
        stack.push((a, ma, depth + 1));
        stack.push((b, mb, depth + 1));
    }
    let ratio = Interval::point(worst)
        .div(&u_c)
        .ok_or("DegenerateUncertaintyError")?;
    Ok(Adequacy {
        verdict,
        error: error.ok_or("EmptyModelError")?,
        u_c_hi: u_c.hi,
        ratio_hi: ratio.hi,
        threshold,
        boxes_used: boxes,
        budget,
        max_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: &[IDual]) -> Result<IDual, E> {
        Ok(x[0].mul(&x[0]))
    }

    #[test]
    fn arithmetic_encloses_exact_results() {
        let a = Interval::point(0.1);
        let b = Interval::point(0.2);
        let s = a.add(&b);
        // exact decimal 0.1 + 0.2 as the doubles below sum to is inside.
        assert!(s.lo < s.hi && s.contains(0.1 + 0.2));
        let t = Interval::point(1.0).div(&Interval::point(3.0)).unwrap();
        assert!(t.lo < 1.0 / 3.0 && 1.0 / 3.0 < t.hi);
        assert!(Interval::new(-1.0, 1.0)
            .div(&Interval::new(-1.0, 1.0))
            .is_none());
        let r = Interval::point(2.0).sqrt(0).unwrap();
        assert!(r.lo * r.lo <= 2.0 && 2.0 <= r.hi * r.hi);
        let e = Interval::point(1.0).exp(0).unwrap();
        assert!(e.contains(std::f64::consts::E));
        let l = Interval::point(std::f64::consts::E).ln(0).unwrap();
        assert!(l.contains(1.0));
    }

    #[test]
    fn mul_sign_cases_and_quantize_identity() {
        let p = Interval::new(-2.0, 3.0).mul(&Interval::new(-5.0, 4.0));
        assert!(p.lo <= -15.0 && p.hi >= 12.0);
        let x = Interval::new(1.0, 2.0);
        assert_eq!(x.quantize(3), x);
    }

    #[test]
    fn dual_gradient_is_enclosed() {
        let x = Interval::new(9.99, 10.01);
        let r = sq(&[IDual::variable(x, true)]).unwrap();
        assert!(r.d.contains(20.0) && r.d.lo <= 19.98 && r.d.hi >= 20.02);
    }

    #[test]
    fn adequate_small_curvature() {
        let r = lpu_adequacy(&sq, &[10.0], &[0.001], 1.0, 0.05, 64).unwrap();
        assert_eq!(r.verdict, Verdict::Adequate);
        assert!(r.ratio_hi < 1e-3);
    }

    #[test]
    fn inadequate_large_curvature() {
        // x^2 at 1 +/- 1: error reaches 1 against u_c = 2.
        let r = lpu_adequacy(&sq, &[1.0], &[1.0], 1.0, 0.05, 64).unwrap();
        assert_eq!(r.verdict, Verdict::Inadequate);
    }

    #[test]
    fn budget_exhaustion_is_inconclusive_and_recorded() {
        // True worst error is 0.5 * u_c = 1 -> ratio 0.5; threshold 0.5 sits
        // on the boundary so the proof cannot close with a tiny budget.
        let r = lpu_adequacy(&sq, &[1.0], &[1.0], 1.0, 0.5, 1).unwrap();
        assert_eq!(r.verdict, Verdict::Inconclusive);
        assert_eq!(r.boxes_used, 1);
        assert_eq!(r.budget, 1);
    }

    #[test]
    fn enclosure_decides_where_plain_f64_is_ambiguous() {
        // y = x^2 at 1e8, u = 1e-9: true linearization error is u^2 = 1e-18.
        // In f64, (c+u)^2 - c^2 - 2cu cancels 1e16-sized terms, so the naive
        // estimate is pure rounding noise (0 or a multiple of the ulp), not
        // a usable bound against the 1e-2 limit.
        let (c, u) = (1.0e8_f64, 1.0e-9_f64);
        let naive = (c + u) * (c + u) - c * c - 2.0 * c * u;
        let limit = 0.05 * 2.0 * c * u;
        assert!(naive == 0.0 || naive.abs() > 1e-12, "naive={naive}");
        let r = lpu_adequacy(&sq, &[c], &[u], 1.0, 0.05, 64).unwrap();
        assert_eq!(r.verdict, Verdict::Adequate);
        assert!(r.error.mag() < 1e-9 && r.error.mag() < limit);
    }

    #[test]
    fn json_records_budget() {
        let r = lpu_adequacy(&sq, &[10.0], &[0.001], 1.0, 0.05, 64).unwrap();
        let j = r.to_json();
        assert_eq!(j["subdivision"]["budget"], 64);
        assert_eq!(j["verdict"], "adequate");
    }
}
