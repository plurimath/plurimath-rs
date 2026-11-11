//! `Formatter::Numbers::Integer`, `Fraction` and `Significant`.

use super::digits::{chars_of, decimal_to_base, fraction_change_base, DigitSequence};
use super::options::{FormatOptions, HexCapital};
use super::source::Parts;
use super::{FormatterError, MAX_EXPANSION};

fn capitalize_hex_digits(base: u32, hex: HexCapital, s: &str) -> String {
    if base == 16 && hex == HexCapital::NumbersOnly {
        s.chars().map(|c| if ('a'..='f').contains(&c) { c.to_ascii_uppercase() } else { c }).collect()
    } else {
        s.to_string()
    }
}

fn expansion(n: i64) -> Result<usize, FormatterError> {
    if n as u64 > MAX_EXPANSION as u64 {
        return Err(FormatterError::ResourceLimit { what: "digit expansion" });
    }
    Ok(n.max(0) as usize)
}

/// `Formatter::Numbers::Integer`.
pub(crate) struct IntegerFormat {
    base: u32,
    hex: HexCapital,
    groups: i64,
    separator: String,
    padding: String,
    padding_digits: i64,
    padding_group_digits: i64,
}

impl IntegerFormat {
    pub(crate) fn new(options: &FormatOptions, base: u32) -> Result<Self, FormatterError> {
        Ok(IntegerFormat {
            base,
            groups: options.group_digits()?,
            separator: options.group()?,
            padding: options.padding()?,
            padding_digits: options.padding_digits()?,
            padding_group_digits: options.padding_group_digits()?,
            hex: options.hex_capital()?,
        })
    }

    pub(crate) fn number_to_base(&self, number: &str) -> String {
        if self.base == 10 {
            number.to_string()
        } else {
            decimal_to_base(number, self.base)
        }
    }

    pub(crate) fn format_groups(&self, string: &str) -> Result<String, FormatterError> {
        let string = capitalize_hex_digits(self.base, self.hex, string);
        let mut chars: Vec<char> = string.chars().collect();
        let width = self.padding_target_width(chars.len() as i64);
        if width > chars.len() as i64 {
            let pad = self.padding.chars().next().unwrap_or('0');
            let extra = expansion(width)? - chars.len();
            let mut padded = vec![pad; extra];
            padded.extend(chars);
            chars = padded;
        }
        if self.groups <= 0 {
            return Ok(chars.into_iter().collect());
        }
        let size = self.groups.min(chars.len().max(1) as i64) as usize;
        let mut tokens: Vec<String> = Vec::new();
        let mut end = chars.len();
        while end > 0 {
            let start = end.saturating_sub(size);
            tokens.push(chars[start..end].iter().collect());
            end = start;
        }
        tokens.reverse();
        Ok(tokens.join(&self.separator))
    }

    fn padding_target_width(&self, len: i64) -> i64 {
        if self.padding_digits > 0 {
            return self.padding_digits;
        }
        if self.padding_group_digits <= 0 {
            return len;
        }
        let remainder = len % self.padding_group_digits;
        if remainder == 0 {
            len
        } else {
            len + self.padding_group_digits - remainder
        }
    }
}

/// `Formatter::Numbers::Fraction`.
pub(crate) struct FractionFormat {
    base: u32,
    hex: HexCapital,
    seq: DigitSequence,
    group: Option<i64>,
    pub(crate) decimal: String,
    separator: String,
    digit_count: i64,
    /// `@integer_digits` state carried through `apply_parts`.
    integer_digits: String,
}

impl FractionFormat {
    pub(crate) fn new(options: &FormatOptions, base: u32) -> Result<Self, FormatterError> {
        Ok(FractionFormat {
            base,
            seq: DigitSequence::new(base),
            group: options.fraction_group_digits()?,
            decimal: options.decimal()?.to_s(),
            separator: options.fraction_group()?,
            digit_count: options.digit_count()?,
            hex: options.hex_capital()?,
            integer_digits: String::new(),
        })
    }

    pub(crate) fn apply_parts(&mut self, parts: &Parts, precision: i64) -> Result<Parts, FormatterError> {
        if precision <= 0 {
            return Ok(parts.with_digits(None, Some("")));
        }
        self.integer_digits = parts.integer_digits.clone();
        let mut fraction = parts.fraction_digits.clone();
        if self.base != 10 && fraction.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
            fraction = fraction_change_base(&fraction, self.base, expansion(precision)?);
        }
        let number = if self.digit_count > 0 {
            self.digit_count_format(&fraction)?
        } else {
            let len = fraction.chars().count();
            let p = expansion(precision)?;
            if p <= len {
                fraction
            } else {
                fraction + &"0".repeat(p - len)
            }
        };
        let integer = self.integer_digits.clone();
        Ok(parts.with_digits(Some(&integer), Some(&number)))
    }

    pub(crate) fn format_groups(&self, string: &str) -> String {
        let string = capitalize_hex_digits(self.base, self.hex, string);
        let chars: Vec<char> = string.chars().collect();
        let length = match self.group {
            Some(g) if g != 0 => g as usize,
            _ => chars.len(),
        };
        if chars.is_empty() {
            return String::new();
        }
        chars.chunks(length.max(1)).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(&self.separator)
    }

    fn digit_count_format(&mut self, fraction: &str) -> Result<String, FormatterError> {
        let int_length = (self.integer_digits.chars().count() + fraction.chars().count()) as i64;
        if int_length > self.digit_count {
            if self.digit_count <= self.integer_digits.chars().count() as i64 {
                let first = fraction.chars().next().map(|c| c.to_string());
                if self.seq.round_up(first.as_deref()) {
                    self.round_integer(&mut Vec::new(), 1);
                }
                return Ok(String::new());
            }
            Ok(self.round_base_string(fraction))
        } else if int_length < self.digit_count {
            Ok(format!("{}{}", fraction, "0".repeat(expansion(self.digit_count - int_length)?)))
        } else {
            Ok(fraction.to_string())
        }
    }

    fn round_base_string(&mut self, fraction: &str) -> String {
        let keep = (self.digit_count - self.integer_digits.chars().count() as i64) as usize;
        let mut digits: Vec<String> = chars_of(fraction).into_iter().take(keep + 1).collect();
        let discard = match digits.pop() {
            Some(d) => d,
            None => return String::new(),
        };
        if !self.seq.round_up(Some(&discard)) {
            return digits.concat();
        }
        let reversed: Vec<String> = digits.into_iter().rev().collect();
        let (mut rounded_reversed, carry) = self.seq.increment_reversed(&reversed, 1, "0");
        if carry > 0 {
            self.round_integer(&mut rounded_reversed, carry);
        }
        rounded_reversed.reverse();
        rounded_reversed.concat()
    }

    fn round_integer(&mut self, fraction_digits_reversed: &mut Vec<String>, carry: u32) {
        let reversed: Vec<String> = chars_of(&self.integer_digits).into_iter().rev().collect();
        let (incremented, carry) = self.seq.increment_reversed(&reversed, carry, "0");
        let mut incremented: Vec<String> = incremented;
        incremented.reverse();
        let mut new_integer = incremented.concat();
        if carry > 0 {
            fraction_digits_reversed.pop();
            new_integer.insert(0, '1');
        }
        self.integer_digits = new_integer;
    }
}

/// `Formatter::Numbers::Significant`.
pub(crate) struct Significant {
    seq: DigitSequence,
    pub(crate) significant: i64,
}

const DOT: &str = ".";

impl Significant {
    pub(crate) fn new(options: &FormatOptions, base: u32) -> Result<Self, FormatterError> {
        Ok(Significant { seq: DigitSequence::new(base), significant: options.significant()? })
    }

    pub(crate) fn is_active(&self) -> bool {
        self.significant > 0
    }

    pub(crate) fn apply_parts(&self, parts: &Parts) -> Result<Parts, FormatterError> {
        let string = if parts.fraction_digits.is_empty() {
            parts.integer_digits.clone()
        } else {
            format!("{}.{}", parts.integer_digits, parts.fraction_digits)
        };
        let chars = chars_of(&string);
        if self.skip(&chars) {
            return Ok(parts.clone());
        }
        let signified = self.signify(&chars)?;
        let (integer, fraction) = match signified.split_once('.') {
            Some((i, f)) => (i.to_string(), f.to_string()),
            None => (signified, String::new()),
        };
        Ok(parts.with_digits(Some(&integer), Some(&fraction)))
    }

    fn sig(&self) -> usize {
        self.significant as usize
    }

    fn skip(&self, chars: &[String]) -> bool {
        self.seq.significant_digit_count(chars) == 0 || self.seq.digit_count(chars, None) == self.sig()
    }

    fn signify(&self, chars: &[String]) -> Result<String, FormatterError> {
        let (mut new_chars, frac_part, sig_count) = self.process_chars(chars);
        if sig_count > 0 {
            if !frac_part {
                new_chars.push(DOT.into());
            }
        } else {
            let stop = if frac_part { None } else { Some(DOT) };
            let mut remain = self.seq.digit_count(chars, stop) as i64 - self.significant;
            if remain > 0 {
                self.round_chars(chars, &mut new_chars, frac_part);
                if frac_part {
                    remain = self.remaining_fraction_chars(&new_chars);
                }
            }
            if !(frac_part && self.seq.significant_digit_count(&new_chars) == self.sig()) {
                new_chars.push("0".repeat(expansion(remain)?));
            }
        }
        Ok(new_chars.concat())
    }

    fn round_chars(&self, chars: &[String], result: &mut Vec<String>, frac_part: bool) {
        let len = result.len();
        let index = if self.seq.is_digit(chars.get(len).map(String::as_str)) { len } else { len + 1 };
        if !(index < chars.len() && self.seq.round_up(Some(&chars[index]))) {
            return;
        }
        let reversed: Vec<String> = result.iter().rev().cloned().collect();
        let mut rounded = if frac_part { self.increment_fractional(reversed) } else { self.increment_integer(reversed) };
        rounded.reverse();
        *result = rounded;
    }

    fn increment_fractional(&self, reversed: Vec<String>) -> Vec<String> {
        let Some(di) = reversed.iter().position(|c| c == DOT) else {
            return self.increment_integer(reversed);
        };
        let fraction = &reversed[..di];
        let integer = reversed[di + 1..].to_vec();
        let (mut fraction, carry) = self.seq.increment_reversed(fraction, 1, "");
        if carry == 0 {
            fraction.push(DOT.into());
            fraction.extend(integer);
            return fraction;
        }
        let integer = self.increment_integer(integer);
        fraction.push(String::new());
        fraction.extend(integer);
        fraction
    }

    fn increment_integer(&self, reversed: Vec<String>) -> Vec<String> {
        let (mut digits, carry) = self.seq.increment_reversed(&reversed, 1, "0");
        if carry > 0 {
            digits.push("1".into());
        }
        digits
    }

    fn remaining_fraction_chars(&self, chars: &[String]) -> i64 {
        if !chars.iter().any(|c| c == DOT) {
            return 0;
        }
        (self.significant - self.seq.significant_digit_count(chars) as i64).max(0)
    }

    fn process_chars(&self, chars: &[String]) -> (Vec<String>, bool, i64) {
        let mut sig_count = self.significant;
        let mut new_chars = Vec::new();
        let mut frac_part = false;
        let mut sig_num = false;
        for c in chars {
            frac_part = frac_part || c == DOT;
            sig_num = sig_num || self.seq.is_significant(c);
            if sig_count == 0 {
                break;
            }
            new_chars.push(c.clone());
            if !sig_num || !self.seq.is_digit(Some(c)) {
                continue;
            }
            sig_count -= 1;
        }
        (new_chars, frac_part, sig_count)
    }
}
