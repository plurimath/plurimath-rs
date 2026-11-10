//! Decimal/BigMath port from pinned plurimath-ts big-decimal.ts/big-math.ts.
//! Significant-digit half-up rounding, binary splitting, Newton and AGM order
//! follow that source. Source pins are recorded in the local R30 receipts.
use crate::{digits, json, pow10, BigInt, E, J, N, Q};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};
#[derive(Clone)]
pub(crate) struct D {
    pub c: BigInt,
    pub s: i64,
    pub nz: bool,
}
impl D {
    pub fn new(mut c: BigInt, mut s: i64, nz: bool) -> Self {
        if c.is_zero() {
            return Self { c, s: 0, nz };
        }
        while (&c % 10u8).is_zero() {
            c /= 10u8;
            s += 1
        }
        Self { c, s, nz: false }
    }
    fn int(n: i64) -> Self {
        Self::new(n.into(), 0, false)
    }
    pub(crate) fn negative(&self) -> bool {
        self.c.is_negative() || self.nz
    }
    fn zero(&self) -> bool {
        self.c.is_zero()
    }
    fn exp(&self) -> i64 {
        if self.zero() {
            0
        } else {
            digits(&self.c) + self.s
        }
    }
    fn significant(&self) -> u32 {
        if self.zero() {
            0
        } else {
            digits(&self.c) as u32
        }
    }
    pub fn precision(&self) -> u32 {
        if self.zero() {
            0
        } else {
            let n = digits(&self.c);
            let e = n + self.s;
            if e > 0 {
                n.max(e) as u32
            } else {
                (n - e) as u32
            }
        }
    }
    fn words(&self) -> i64 {
        if self.zero() {
            1
        } else {
            let n = digits(&self.c);
            let e = n + self.s;
            (e - 1).div_euclid(9) + 1 - (e - n).div_euclid(9)
        }
    }
    fn coerce(&self) -> u32 {
        (self.words() as u32 * 9).max(32)
    }
    pub fn q(&self) -> Q {
        if self.s >= 0 {
            Q::from_integer(&self.c * pow10(self.s as u32))
        } else {
            Q::new(self.c.clone(), pow10((-self.s) as u32))
        }
    }
    fn fromq(q: &Q, p: u32) -> Self {
        match crate::roundq(q, p) {
            N::Decimal(d) => d,
            _ => unreachable!(),
        }
    }
    fn float(x: f64) -> Result<Self, E> {
        if !x.is_finite() {
            return Err("NonFiniteResultError");
        };
        if x == 0.0 {
            return Ok(Self::new(0.into(), 0, x.is_sign_negative()));
        };
        let q = crate::qparse(&x.to_string())?;
        let mut d = Self::new(q.numer().clone(), 0, false);
        if !q.denom().is_one() {
            let text = x.to_string();
            let parts: Vec<_> = text.split(['e', 'E']).collect();
            let exp: i64 = if parts.len() == 2 {
                parts[1].parse().unwrap()
            } else {
                0
            };
            let after = parts[0].split('.').nth(1).map_or(0, |s| s.len() as i64);
            d = Self::new(
                parts[0].replace('.', "").parse().unwrap(),
                exp - after,
                false,
            )
        }
        let n = digits(&d.c);
        if n > 16 {
            let drop = n - 16;
            d = Self::new(d.c / pow10(drop as u32), d.s + drop, false)
        }
        Ok(d)
    }
    pub(crate) fn f(&self) -> f64 {
        if self.zero() {
            return if self.nz { -0.0 } else { 0.0 };
        }
        self.q().to_f64().unwrap_or(if self.negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    }
    pub fn wire(&self) -> J {
        json!({"kind":"decimal","value":if self.zero(){if self.nz{"-0.0"}else{"0.0"}.to_owned()}else{format!("{}0.{}e{}",if self.negative(){"-"}else{""},self.c.abs(),self.exp())},"precision":self.precision()})
    }
    pub fn neg(&self) -> Self {
        Self::new(
            -self.c.clone(),
            self.s,
            if self.zero() { !self.nz } else { false },
        )
    }
    fn abs(&self) -> Self {
        Self::new(self.c.abs(), self.s, false)
    }
    fn round(&self, p: u32) -> Self {
        if p == 0 || self.zero() || self.significant() <= p {
            return self.clone();
        };
        Self::fromq(&self.q(), p)
    }
    fn add(&self, b: &Self, p: u32) -> Self {
        if self.zero() && b.zero() {
            return Self::new(0.into(), 0, self.negative() && b.negative());
        };
        let s = self.s.min(b.s);
        Self::new(
            &self.c * pow10((self.s - s) as u32) + &b.c * pow10((b.s - s) as u32),
            s,
            false,
        )
        .round(p)
    }
    fn sub(&self, b: &Self, p: u32) -> Self {
        self.add(&b.neg(), p)
    }
    fn mul(&self, b: &Self, p: u32) -> Self {
        Self::new(
            &self.c * &b.c,
            self.s + b.s,
            self.negative() != b.negative(),
        )
        .round(p)
    }
    fn div(&self, b: &Self, p: u32) -> Self {
        assert!(!b.zero());
        if self.zero() {
            return Self::new(0.into(), 0, self.negative() != b.negative());
        };
        Self::fromq(&(self.q() / b.q()), p)
    }
    fn quo(&self, b: &Self) -> Self {
        self.div(b, (self.precision().max(b.precision()) + 16).max(32))
    }
    fn shift(&self, n: i64) -> Self {
        Self::new(self.c.clone(), self.s + n, self.nz)
    }
    fn truncate(&self, n: i64) -> Self {
        if self.zero() || self.s >= -n {
            return self.clone();
        };
        let drop = -n - self.s;
        Self::new(&self.c / pow10(drop as u32), self.s + drop, self.negative())
    }
    fn floor(&self) -> BigInt {
        self.q().numer().div_floor(self.q().denom())
    }
    fn compare(&self, b: &Self) -> std::cmp::Ordering {
        self.q().cmp(&b.q())
    }
    fn sqrt(&self, p: u32) -> Result<Self, E> {
        if self.negative() && !self.zero() {
            return Err("NonFiniteResultError");
        };
        if self.zero() {
            return Ok(self.clone());
        };
        let e = self.exp().div_euclid(2);
        let reduced = self.shift(-2 * e);
        let mut y = Self::float(reduced.f().sqrt())?;
        for n in newton(p + 16) {
            y = y.add(&reduced.div(&y, n), n).div(&Self::int(2), n)
        }
        Ok(y.shift(e).round(p))
    }
}
fn newton(p: u32) -> Vec<u32> {
    let mut ps = vec![];
    let mut n = p;
    while n > 8 {
        ps.push(n);
        n = (n + 1) / 2 + 2
    }
    ps.reverse();
    ps
}
fn float_log(x: &D) -> f64 {
    x.shift(-x.exp()).f().ln() + x.exp() as f64 * 10f64.ln()
}
fn log_gamma(n: usize) -> f64 {
    (2..n).map(|i| (i as f64).ln()).sum()
}
fn first_true(f: impl Fn(usize) -> bool) -> usize {
    let mut diff = 1;
    let mut hi = 2;
    while !f(hi) {
        diff *= 2;
        hi = 1 + diff
    }
    let mut lo = 1;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if f(mid) {
            hi = mid
        } else {
            lo = mid + 1
        }
    }
    lo
}
fn taylor(x: &D, ds: &[i64], p: u32) -> D {
    let mut fs: Vec<(D, D)> = ds.iter().map(|d| (D::int(0), D::int(*d))).collect();
    let mut xn: Option<D> = None;
    while fs.len() > 1 {
        let power = xn.as_ref().map_or_else(|| x.clone(), |n| n.mul(n, p));
        xn = Some(power.clone());
        let mut next = vec![];
        for pair in fs.chunks(2) {
            let a = &pair[0];
            let ghost = (D::int(0), D::int(1).shift(power.exp().max(0) + 2));
            let b = pair.get(1).unwrap_or(&ghost);
            next.push((
                a.0.mul(&b.1, 0).add(&power.mul(&b.0.add(&b.1, 0), 0), p),
                a.1.mul(&b.1, p),
            ))
        }
        fs = next
    }
    fs[0].0.div(&fs[0].1, p)
}
fn exp_split(x: &D, p: u32) -> D {
    if x.zero() {
        return D::int(1);
    }
    let logx = float_log(&x.abs());
    let step = first_true(|k| log_gamma(k + 1) - k as f64 * logx > p as f64 * 10f64.ln());
    D::int(1).add(
        &taylor(x, &(1..=step).map(|i| i as i64).collect::<Vec<_>>(), p),
        0,
    )
}
fn exp(x: &D, p: u32) -> Result<D, E> {
    if x.exp() >= 21 {
        return if x.negative() {
            Ok(D::int(0))
        } else {
            Err("NonFiniteResultError")
        };
    }
    if x.zero() {
        return Ok(D::int(1));
    };
    let cnt = if x.compare(&D::int(-1)).is_lt() || x.compare(&D::int(1)).is_gt() {
        x.exp()
    } else {
        0
    };
    let n = p + 16 + cnt as u32;
    let mut rest = x.shift(-cnt).round(n);
    let mut digits = 2;
    let mut y = D::int(1);
    while !rest.zero() {
        let partial = rest.truncate(digits);
        rest = rest.sub(&partial, 0);
        y = y.mul(&exp_split(&partial, n), n);
        digits *= 2
    }
    for _ in 0..cnt {
        let y2 = y.mul(&y, n);
        let y5 = y2.mul(&y2, n).mul(&y, n);
        y = y5.mul(&y5, n)
    }
    Ok(y.round(p))
}
fn log(x: &D, p: u32) -> Result<D, E> {
    if x.negative() && !x.zero() {
        return Err("MathDomainError");
    };
    if x.zero() {
        return Err("NonFiniteResultError");
    };
    if x.compare(&D::int(1)).is_eq() {
        return Ok(D::int(0));
    };
    let n = p + 16;
    let mut reduced = x.clone();
    let mut approximation = D::int(0);
    if x.compare(&D::float(1.01)?).is_gt() || x.compare(&D::float(0.99)?).is_lt() {
        approximation = D::float(float_log(x))?;
        reduced = x.div(&exp(&approximation, n)?, n)
    }
    let mut y = D::float(float_log(&reduced))?;
    let extra = (-reduced.sub(&D::int(1), 0).exp()).max(0) as u32;
    for prec in newton(n) {
        let ey = exp(&y, prec + extra)?;
        y = y.sub(&ey.sub(&reduced, prec).div(&ey, prec), prec)
    }
    Ok(y.add(&approximation, p))
}
fn pi(p: u32) -> Result<D, E> {
    let n = p + 16;
    let mut a = D::int(1);
    let mut b = D::float(0.5)?.sqrt(n)?;
    let mut s = D::float(0.25)?;
    let mut t = BigInt::one();
    while a.compare(&b).is_ne() && a.sub(&b, 0).exp() > 1 - n as i64 {
        let c = a.sub(&b, 0).div(&D::int(2), n);
        let next = a.add(&b, 0).div(&D::int(2), n);
        b = a.mul(&b, 0).sqrt(n)?;
        a = next;
        s = s.sub(&c.mul(&c, 0).mul(&D::new(t.clone(), 0, false), 0), n);
        t *= 2u8
    }
    Ok(a.mul(&b, 0).div(&s, p))
}
fn sin_reduce(x: &D, p: u32, half: bool) -> Result<(i64, D), E> {
    let bound = D::float(std::f64::consts::FRAC_PI_2)?;
    if !half && x.compare(&bound.neg()).is_ge() && x.compare(&bound).is_le() {
        return Ok((1, x.clone()));
    };
    let mut mp = p + 16;
    let extra = x.exp().max(0) as u32 + 16;
    loop {
        let pi = pi(mp + extra)?;
        let hp = pi.quo(&D::int(2));
        let shifted = x.add(if half { &pi } else { &hp }, 0);
        let quotient = shifted
            .div(&pi, (shifted.significant() + pi.significant() + 32).max(32))
            .floor();
        let remainder = shifted.sub(&pi.mul(&D::new(quotient.clone(), 0, false), 0), 0);
        let m = remainder.sub(&hp, 0);
        if m.zero() || mp as i64 + m.exp() <= 0 {
            mp = mp * 3 / 2 + 16
        } else if mp as i64 + m.exp() < p as i64 {
            mp = (p as i64 - m.exp()) as u32 + 16
        } else {
            return Ok((if quotient.is_even() { 1 } else { -1 }, m.round(p)));
        }
    }
}
fn sin_split(x: &D, p: u32) -> D {
    if x.zero() {
        return x.clone();
    };
    let x2 = x.mul(x, p);
    let logx = float_log(&x.abs());
    let step = first_true(|k| log_gamma(2 * k + 1) - 2.0 * k as f64 * logx > p as f64 * 10f64.ln());
    let ds = (1..=step)
        .map(|k| -((2 * k) as i64) * (2 * k + 1) as i64)
        .collect::<Vec<_>>();
    x.mul(&D::int(1).add(&taylor(&x2, &ds, p), 0), p)
}
fn sin_zero(x: &D, p: u32) -> Result<D, E> {
    let mut sine = D::int(0);
    let mut cosine = D::int(1);
    let mut rest = x.clone();
    let mut n = 2;
    while !rest.zero() {
        let partial = rest.truncate(n);
        rest = rest.sub(&partial, 0);
        let s = sin_split(&partial, p);
        let c = D::int(1).sub(&s.mul(&s, 0), 0).sqrt(p)?;
        let next = sine.mul(&c, 0).add(&cosine.mul(&s, 0), p);
        cosine = cosine.mul(&c, 0).sub(&sine.mul(&s, 0), p);
        sine = next;
        n *= 2
    }
    if sine.compare(&D::int(-1)).is_lt() {
        Ok(D::int(-1))
    } else if sine.compare(&D::int(1)).is_gt() {
        Ok(D::int(1))
    } else {
        Ok(sine)
    }
}
fn trig(x: &D, p: u32, cosine: bool) -> Result<D, E> {
    let n = p + 16;
    let (sign, reduced) = sin_reduce(x, n, cosine)?;
    Ok(sin_zero(&reduced, n)?.mul(&D::int(sign), p))
}
fn power(x: &D, y: &D, p: u32) -> Result<D, E> {
    if y.zero() {
        return Ok(D::int(1));
    };
    let integral = y.q().is_integer();
    if x.zero() {
        return if y.negative() {
            Err("NonFiniteResultError")
        } else {
            Ok(D::int(0))
        };
    }
    if x.negative() {
        if !integral {
            return Err("MathDomainError");
        };
        let out = power(&x.neg(), y, p)?;
        return Ok(if y.q().to_integer().is_odd() {
            out.neg()
        } else {
            out
        });
    }
    if x.compare(&D::int(1)).is_eq() {
        return Ok(D::int(1));
    };
    if integral && p == 0 {
        let n = y
            .q()
            .to_integer()
            .to_i32()
            .ok_or("UnsupportedExpressionError")?;
        if n.unsigned_abs() > 100000 {
            return Err("UnsupportedExpressionError");
        };
        let out = D::new(
            x.c.pow(n.unsigned_abs()),
            x.s * n.unsigned_abs() as i64,
            false,
        );
        return Ok(if n < 0 { D::int(1).quo(&out) } else { out });
    };
    let result = if p > 0 {
        p
    } else {
        x.significant().max(y.significant()).max(16) + 16
    };
    let mut n = result + 16;
    if y.negative() {
        return Ok(D::int(1).div(&power(x, &y.neg(), n)?, result));
    };
    if integral && (y.exp() as f64) < (result as f64).ln() * 5.0 + 20.0 {
        let powprec = n + y.exp().max(0) as u32;
        let mut whole = y
            .q()
            .to_integer()
            .to_u64()
            .ok_or("UnsupportedExpressionError")?;
        let mut base = x.clone();
        let mut answer = D::int(1);
        while whole > 0 {
            if whole % 2 == 1 {
                answer = answer.mul(&base, powprec)
            }
            whole /= 2;
            if whole > 0 {
                base = base.mul(&base, powprec)
            }
        }
        return Ok(answer.round(result));
    };
    if x.compare(&D::int(1)).is_gt() {
        let le = if x.compare(&D::int(2)).is_lt() {
            x.sub(&D::int(1), 0).exp()
        } else {
            (x.exp() as f64).log10().round() as i64
        };
        n += (y.exp() + le).max(0) as u32
    }
    exp(&log(x, n)?.mul(y, n), result)
}
fn convert(x: &N, other: &D, exponent: bool) -> Result<D, E> {
    match x {
        N::Decimal(d) => Ok(d.clone()),
        N::Exact(q, true) => Ok(D::new(q.numer().clone(), 0, false)),
        N::Exact(q, false) => Ok(D::fromq(
            q,
            if exponent {
                other.significant().max(32)
            } else {
                other.coerce()
            },
        )),
        N::Float(f) => D::float(*f),
    }
}
pub(crate) fn guard(op: &str, value: &N, p: u32) -> Result<N, E> {
    let x = match value {
        N::Exact(q, true) => D::new(q.numer().clone(), 0, false),
        N::Exact(q, false) => D::fromq(q, p),
        N::Decimal(d) => d.clone(),
        N::Float(f) => D::float(*f)?,
    };
    Ok(N::Decimal(match op {
        "sqrt" => x.sqrt(p)?,
        "sin" => trig(&x, p, false)?,
        "cos" => trig(&x, p, true)?,
        "exp" => exp(&x, p)?,
        "log" => log(&x, p)?,
        _ => return Err("UnsupportedExpressionError"),
    }))
}
pub(crate) fn binary(op: &str, left: &N, right: &N) -> Result<N, E> {
    let (a, b) = match (left, right) {
        (N::Decimal(d), _) => (d.clone(), convert(right, d, op == "^")?),
        (_, N::Decimal(d)) => (convert(left, d, false)?, d.clone()),
        _ => unreachable!(),
    };
    Ok(N::Decimal(match op {
        "+" => a.add(&b, 0),
        "-" => a.sub(&b, 0),
        "*" => a.mul(&b, 0),
        "/" => a.quo(&b),
        "^" => power(&a, &b, 0)?,
        _ => return Err("UnsupportedExpressionError"),
    }))
}
