//! Exact digit arithmetic (no floating point) and the Ruby
//! `Formatter::Numbers::DigitSequence` helper.

/// `Base::HEX_ALPHANUMERIC`.
pub(crate) const HEX_ALPHANUMERIC: [char; 16] =
    ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'];

/// `Base::DIGIT_VALUE[s]` — only single lowercase hex characters are digits.
pub(crate) fn digit_value(s: &str) -> Option<u32> {
    let mut it = s.chars();
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        'a'..='f' => Some(c as u32 - 'a' as u32 + 10),
        _ => None,
    }
}

const LIMB: u64 = 1_000_000_000;

/// Big-endian base-1e9 limbs of a decimal digit string (non-digits ignored).
fn decimal_to_limbs(dec: &str) -> Vec<u64> {
    let digits: Vec<u8> = dec.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    let start = digits.iter().position(|d| *d != 0).unwrap_or(digits.len());
    let digits = &digits[start..];
    let mut limbs = Vec::with_capacity(digits.len() / 9 + 1);
    let head = digits.len() % 9;
    let mut i = 0;
    if head > 0 {
        limbs.push(digits[..head].iter().fold(0u64, |a, d| a * 10 + *d as u64));
        i = head;
    }
    while i < digits.len() {
        limbs.push(digits[i..i + 9].iter().fold(0u64, |a, d| a * 10 + *d as u64));
        i += 9;
    }
    limbs
}

fn limbs_to_decimal(limbs: &[u64]) -> String {
    let start = limbs.iter().position(|l| *l != 0).unwrap_or(limbs.len());
    if start == limbs.len() {
        return "0".into();
    }
    let mut out = limbs[start].to_string();
    for l in &limbs[start + 1..] {
        out.push_str(&format!("{l:09}"));
    }
    out
}

/// Ruby `Integer#to_s(base)` applied to `decimal_digits.to_i` (lowercase).
pub(crate) fn decimal_to_base(dec: &str, base: u32) -> String {
    let mut limbs = decimal_to_limbs(dec);
    if limbs.is_empty() {
        return "0".into();
    }
    if base == 10 {
        return limbs_to_decimal(&limbs);
    }
    // Divide by base^k chunks so each pass yields k digits.
    let mut k = 1u32;
    let mut chunk = base as u64;
    while chunk * (base as u64) <= (1 << 30) {
        chunk *= base as u64;
        k += 1;
    }
    let mut out: Vec<char> = Vec::new();
    while !limbs.is_empty() {
        let mut rem = 0u64;
        let mut q = Vec::with_capacity(limbs.len());
        for l in &limbs {
            let cur = rem * LIMB + l;
            let d = cur / chunk;
            rem = cur % chunk;
            if !(q.is_empty() && d == 0) {
                q.push(d);
            }
        }
        limbs = q;
        for _ in 0..k {
            out.push(HEX_ALPHANUMERIC[(rem % base as u64) as usize]);
            rem /= base as u64;
            if limbs.is_empty() && rem == 0 {
                break;
            }
        }
    }
    while out.len() > 1 && *out.last().unwrap() == '0' {
        out.pop();
    }
    out.iter().rev().collect()
}

/// Exact decimal digits of the integer whose base-`base` digit values are
/// `values` (most significant first).
pub(crate) fn base_values_to_decimal(values: &[u32], base: u32) -> String {
    let mut limbs: Vec<u64> = vec![0]; // little-endian
    for v in values {
        let mut carry = *v as u64;
        for l in limbs.iter_mut() {
            let cur = *l * base as u64 + carry;
            *l = cur % LIMB;
            carry = cur / LIMB;
        }
        while carry > 0 {
            limbs.push(carry % LIMB);
            carry /= LIMB;
        }
    }
    limbs.reverse();
    limbs_to_decimal(&limbs)
}

/// `Fraction#change_base`: exact `Rational(fraction.to_i, 10**len)` times
/// `base`, emitting `precision` target-base digits by truncation.
pub(crate) fn fraction_change_base(fraction: &str, base: u32, precision: usize) -> String {
    let mut digits: Vec<u32> = fraction.bytes().filter(u8::is_ascii_digit).map(|b| (b - b'0') as u32).collect();
    let mut out = String::with_capacity(precision);
    for _ in 0..precision {
        let mut carry = 0u32;
        for d in digits.iter_mut().rev() {
            let cur = *d * base + carry;
            *d = cur % 10;
            carry = cur / 10;
        }
        out.push(HEX_ALPHANUMERIC[carry as usize]);
    }
    out
}

/// `Formatter::Numbers::DigitSequence`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DigitSequence {
    pub(crate) base: u32,
}

impl DigitSequence {
    pub(crate) fn new(base: u32) -> Self {
        DigitSequence { base }
    }
    pub(crate) fn is_digit(&self, s: Option<&str>) -> bool {
        s.and_then(digit_value).is_some()
    }
    pub(crate) fn is_significant(&self, s: &str) -> bool {
        digit_value(s).is_some_and(|v| v > 0)
    }
    pub(crate) fn digit_count(&self, chars: &[String], stop_at: Option<&str>) -> usize {
        let mut count = 0;
        for c in chars {
            if stop_at.is_some_and(|s| s == c) {
                break;
            }
            if digit_value(c).is_some() {
                count += 1;
            }
        }
        count
    }
    pub(crate) fn significant_digit_count(&self, chars: &[String]) -> usize {
        let mut started = false;
        let mut count = 0;
        for c in chars {
            if digit_value(c).is_none() {
                continue;
            }
            if self.is_significant(c) {
                started = true;
            }
            if started {
                count += 1;
            }
        }
        count
    }
    pub(crate) fn is_max_digit(&self, s: &str) -> bool {
        digit_value(s) == Some(self.base - 1)
    }
    /// `next_digit` (nil past `f` renders as "" when joined).
    pub(crate) fn next_digit(&self, s: &str) -> String {
        digit_value(s)
            .and_then(|v| HEX_ALPHANUMERIC.get(v as usize + 1))
            .map(|c| c.to_string())
            .unwrap_or_default()
    }
    pub(crate) fn threshold(&self) -> u32 {
        self.base / 2
    }
    pub(crate) fn round_up(&self, s: Option<&str>) -> bool {
        s.and_then(digit_value).is_some_and(|v| v >= self.threshold())
    }
    /// `increment_reversed(digits, carry:, overflow:)` (skip is unused by
    /// every source caller and therefore omitted).
    pub(crate) fn increment_reversed(&self, digits: &[String], carry: u32, overflow: &str) -> (Vec<String>, u32) {
        let mut digits = digits.to_vec();
        if carry == 0 {
            return (digits, carry);
        }
        let mut carry = carry;
        for digit in &mut digits {
            if digit_value(digit).is_none() {
                continue;
            }
            if self.is_max_digit(digit) {
                *digit = overflow.to_string();
            } else {
                *digit = self.next_digit(digit);
                carry = 0;
                break;
            }
        }
        (digits, carry)
    }
}

/// Ruby `String#chars` as owned one-character strings.
pub(crate) fn chars_of(s: &str) -> Vec<String> {
    s.chars().map(|c| c.to_string()).collect()
}
