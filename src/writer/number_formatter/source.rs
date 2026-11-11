//! `Formatter::Numbers::Source` and `Formatter::Numbers::Parts`: exact
//! string magnitude, never a float.

use super::digits::{base_values_to_decimal, decimal_to_base};
use super::value::{string_to_i_digits, string_to_i_lenient, RubyValue};
use super::{FormatterError, MAX_EXPANSION};

/// `Parts`: normalized sign / integer digits / fraction digits.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Parts {
    pub(crate) sign: i32,
    pub(crate) integer_digits: String,
    pub(crate) fraction_digits: String,
}

impl Parts {
    pub(crate) fn new(sign: i32, integer_digits: &str, fraction_digits: &str) -> Parts {
        Parts { sign, integer_digits: normalize_integer(integer_digits), fraction_digits: fraction_digits.to_string() }
    }
    pub(crate) fn is_fractional(&self) -> bool {
        !self.fraction_digits.is_empty()
    }
    pub(crate) fn is_integer_zero(&self) -> bool {
        self.integer_digits == "0"
    }
    pub(crate) fn with_digits(&self, integer_digits: Option<&str>, fraction_digits: Option<&str>) -> Parts {
        Parts::new(
            self.sign,
            integer_digits.unwrap_or(&self.integer_digits),
            fraction_digits.unwrap_or(&self.fraction_digits),
        )
    }
}

/// `value.to_s.sub(/\A0+(?=.)/, "")`, then `"0"` for empty.
fn normalize_integer(value: &str) -> String {
    let stripped = value.trim_start_matches('0');
    let normalized = if stripped.is_empty() && !value.is_empty() {
        // `0+(?=.)` keeps the final character when the run reaches the end.
        &value[value.len() - 1..]
    } else if stripped.len() < value.len() && stripped.starts_with('\n') {
        // `.` does not match a newline, so the last zero before it survives.
        &value[value.len() - stripped.len() - 1..]
    } else {
        stripped
    };
    if normalized.is_empty() {
        "0".into()
    } else {
        normalized.to_string()
    }
}

/// `Source`.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) raw: String,
    pub(crate) sign: i32,
    pub(crate) exponent: i64,
    pub(crate) integer_digits: String,
    pub(crate) fraction_digits: String,
    /// `decimal.zero?`
    pub(crate) zero: bool,
}

/// `\A[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?\z`
pub(crate) fn numeric_pattern(raw: &str) -> bool {
    let b = raw.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_len = i - int_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if int_len == 0 && i == f {
            return false;
        }
    } else if int_len == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let e = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == e {
            return false;
        }
    }
    i == b.len()
}

fn strip_sign(raw: &str) -> &str {
    raw.strip_prefix('-').or_else(|| raw.strip_prefix('+')).unwrap_or(raw)
}

/// `decimal_input?`: `/\A[0-9.]+(?:e[+-]?[0-9]+)?\z/i` on the sign-stripped raw.
fn decimal_input(raw: &str) -> bool {
    let s = strip_sign(raw).as_bytes();
    let mut i = 0;
    while i < s.len() && (s[i].is_ascii_digit() || s[i] == b'.') {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    if i == s.len() {
        return true;
    }
    if s[i] != b'e' && s[i] != b'E' {
        return false;
    }
    i += 1;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    let e = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    i > e && i == s.len()
}

impl Source {
    /// `Source.new(value, base:)` for a String value.
    pub(crate) fn new(raw: &str, base: &RubyValue) -> Result<Source, FormatterError> {
        let raw = raw.to_string();
        // `@base = base || DEFAULT_BASE`
        let base = if base.truthy() { base.clone() } else { RubyValue::Int(10) };
        let non_decimal_base = !base.eq_int(10);
        if !non_decimal_base && !numeric_pattern(&raw) {
            return Err(FormatterError::InvalidNumber { value: raw });
        }
        let sign = if raw.starts_with('-') { -1 } else { 1 };
        let (integer_digits, fraction_digits, exponent) = if decimal_input(&raw) {
            // BigDecimal(raw): the mantissa needs one digit and at most one point.
            let unsigned = strip_sign(&raw).to_ascii_lowercase();
            let (mantissa, exponent_text) = match unsigned.split_once('e') {
                Some((m, e)) => (m.to_string(), Some(e.to_string())),
                None => (unsigned.clone(), None),
            };
            if mantissa.matches('.').count() > 1 || !mantissa.bytes().any(|b| b.is_ascii_digit()) {
                return Err(FormatterError::RubyArgumentError {
                    message: format!("invalid value for BigDecimal(): {:?}", raw),
                });
            }
            let exponent = match exponent_text {
                Some(t) => string_to_i_lenient(&t).ok_or(FormatterError::ResourceLimit { what: "exponent" })?,
                None => 0,
            };
            let (i, f) = match mantissa.split_once('.') {
                Some((i, f)) => (i.to_string(), f.to_string()),
                None => (mantissa.clone(), String::new()),
            };
            (if i.is_empty() { "0".to_string() } else { i }, f, exponent)
        } else {
            // BigDecimal(stripped.to_i(@base))
            let radix = match &base {
                RubyValue::Int(n) => *n,
                RubyValue::Float(t) => t.parse::<f64>().map(|f| f.trunc() as i64).unwrap_or(0),
                other => {
                    return Err(FormatterError::RubyTypeError {
                        message: format!("no implicit conversion of {} into Integer", ruby_class_name(other)),
                    })
                }
            };
            if !(2..=36).contains(&radix) {
                return Err(FormatterError::RubyArgumentError { message: format!("invalid radix {radix}") });
            }
            let (_neg, values) = string_to_i_digits(strip_sign(&raw), radix as u32);
            (base_values_to_decimal(&values, radix as u32), String::new(), 0)
        };
        if exponent.unsigned_abs() as usize > MAX_EXPANSION {
            return Err(FormatterError::ResourceLimit { what: "exponent" });
        }
        let zero = !integer_digits.bytes().chain(fraction_digits.bytes()).any(|b| b != b'0');
        Ok(Source { raw, sign, exponent, integer_digits, fraction_digits, zero })
    }

    /// `fractional?`
    pub(crate) fn is_fractional(&self) -> bool {
        (self.fraction_digits.len() as i64) > self.exponent
    }

    pub(crate) fn decimal_precision(&self) -> i64 {
        self.decimal_digits().1.len() as i64
    }

    pub(crate) fn notation_precision(&self) -> i64 {
        if self.zero {
            return self.fraction_digits.len() as i64;
        }
        (self.significant_digit_count() as i64 - 1).max(0)
    }

    pub(crate) fn significant_digit_count(&self) -> usize {
        let digits = format!("{}{}", self.integer_digits, self.fraction_digits);
        digits.trim_start_matches('0').len()
    }

    pub(crate) fn target_base_integer_length(&self, base: u32) -> usize {
        if base == 10 {
            let parts = self.to_parts(None);
            return if parts.is_integer_zero() { 0 } else { parts.integer_digits.len() };
        }
        let integer = self.decimal_digits().0;
        if !integer.bytes().any(|b| b != b'0') {
            return 0;
        }
        decimal_to_base(&integer, base).len()
    }

    /// `to_parts(precision:)` (the `base:` argument is only carried metadata).
    pub(crate) fn to_parts(&self, precision: Option<i64>) -> Parts {
        let (integer, fraction) = self.decimal_digits();
        let fraction = match precision {
            None => fraction,
            Some(p) if p <= 0 => String::new(),
            Some(p) => fraction.chars().take(p as usize).collect(),
        };
        Parts::new(self.sign, &integer, &fraction)
    }

    fn decimal_digits(&self) -> (String, String) {
        let digits = format!("{}{}", self.integer_digits, self.fraction_digits);
        let decimal_index = self.integer_digits.len() as i64 + self.exponent;
        if decimal_index <= 0 {
            ("0".into(), format!("{}{}", "0".repeat(decimal_index.unsigned_abs() as usize), digits))
        } else if decimal_index as usize >= digits.len() {
            (format!("{}{}", digits, "0".repeat(decimal_index as usize - digits.len())), String::new())
        } else {
            let i = decimal_index as usize;
            (digits[..i].to_string(), digits[i..].to_string())
        }
    }
}

fn ruby_class_name(v: &RubyValue) -> String {
    match v {
        RubyValue::Nil => "nil".into(),
        RubyValue::Bool(true) => "true".into(),
        RubyValue::Bool(false) => "false".into(),
        RubyValue::Int(_) => "Integer".into(),
        RubyValue::Float(_) => "Float".into(),
        RubyValue::Str(_) => "String".into(),
        RubyValue::Sym(_) => "Symbol".into(),
        RubyValue::Other { class, .. } => class.clone(),
    }
}
