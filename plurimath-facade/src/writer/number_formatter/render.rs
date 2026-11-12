//! `BaseNotation`, `FormattedNumber`, `FormattedNotation`, `NumberRenderer`,
//! `NotationRenderer`.

use super::options::{FormatOptions, HexCapital};
use super::source::{Parts, Source};
use super::transforms::{FractionFormat, IntegerFormat, Significant};
use super::value::RubyValue;
use super::FormatterError;

/// `BaseNotation::DEFAULT_PREFIXES`.
pub(crate) fn default_prefix(base: i64) -> Option<&'static str> {
    match base {
        2 => Some("0b"),
        8 => Some("0o"),
        10 => Some(""),
        16 => Some("0x"),
        _ => None,
    }
}

/// `BaseNotation`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BaseNotation {
    pub(crate) base: u32,
    pub(crate) prefix: String,
    pub(crate) postfix: String,
    pub(crate) hex_capital: HexCapital,
    explicit_prefix: bool,
    explicit_postfix: bool,
}

impl BaseNotation {
    pub(crate) fn from_options(options: &FormatOptions) -> Result<Self, FormatterError> {
        let base = options.base();
        let b = match base {
            RubyValue::Int(b) if default_prefix(b).is_some() => b,
            other => return Err(FormatterError::UnsupportedBase { base: other.inspect() }),
        };
        let prefix = if options.has_base_prefix() {
            options.base_prefix()?
        } else if options.has_base_postfix() {
            String::new()
        } else {
            default_prefix(b).unwrap_or_default().to_string()
        };
        let postfix = options.base_postfix()?.to_s();
        let hex_capital = options.hex_capital()?;
        Ok(BaseNotation {
            base: b as u32,
            prefix,
            postfix,
            hex_capital,
            explicit_prefix: options.has_base_prefix(),
            explicit_postfix: options.has_base_postfix(),
        })
    }
    pub(crate) fn is_default(&self) -> bool {
        self.base == 10
    }
    pub(crate) fn is_literal(&self) -> bool {
        !self.is_default() && (self.explicit_prefix || self.explicit_postfix)
    }
    pub(crate) fn is_semantic(&self) -> bool {
        !self.is_default() && !self.is_literal()
    }
    pub(crate) fn upcase_hex(&self) -> bool {
        self.base == 16 && self.hex_capital == HexCapital::True
    }
    pub(crate) fn wrap(&self, digits: &str) -> String {
        format!("{}{}{}", self.prefix, digits, self.postfix)
    }
}

/// `FormattedNumber`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FormattedNumber {
    pub(crate) sign: i32,
    pub(crate) integer_part: String,
    pub(crate) fraction_part: String,
    pub(crate) decimal_separator: String,
    pub(crate) base_notation: BaseNotation,
    pub(crate) number_sign: Option<String>,
}

impl FormattedNumber {
    pub(crate) fn has_base_notation(&self) -> bool {
        !self.base_notation.is_default()
    }
    /// `sign_text`: "-" for negative, "+" for `number_sign: :plus`.
    pub(crate) fn sign_text(&self) -> Option<&'static str> {
        if self.sign == -1 {
            Some("-")
        } else if self.number_sign.as_deref() == Some("plus") {
            Some("+")
        } else {
            None
        }
    }
    /// `digits_string`: digits + decimal separator, hex-upcased, no sign/affixes.
    pub(crate) fn digits_string(&self) -> String {
        let digits = if self.fraction_part.is_empty() {
            self.integer_part.clone()
        } else {
            format!("{}{}{}", self.integer_part, self.decimal_separator, self.fraction_part)
        };
        if self.base_notation.upcase_hex() {
            digits.chars().map(|c| if ('a'..='f').contains(&c) { c.to_ascii_uppercase() } else { c }).collect()
        } else {
            digits
        }
    }
}

impl std::fmt::Display for FormattedNumber {
    /// `FormattedNumber#to_s`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut digits = self.digits_string();
        if !self.base_notation.is_default() {
            digits = self.base_notation.wrap(&digits);
        }
        write!(f, "{}{}", self.sign_text().unwrap_or(""), digits)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NotationStyle {
    E,
    Scientific,
    Engineering,
}

/// `FormattedNotation`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FormattedNotation {
    pub(crate) coefficient: FormattedNumber,
    pub(crate) notation_style: NotationStyle,
    pub(crate) exponent: i64,
    pub(crate) times_symbol: String,
    pub(crate) exponent_separator: String,
    pub(crate) exponent_sign: Option<String>,
}

impl FormattedNotation {
    pub(crate) fn formatted_exponent(&self) -> String {
        if self.exponent == 0 {
            return "0".into();
        }
        let abs = self.exponent.unsigned_abs().to_string();
        if self.exponent < 0 {
            format!("-{abs}")
        } else if self.exponent_sign.as_deref() == Some("plus") {
            format!("+{abs}")
        } else {
            abs
        }
    }
}

impl std::fmt::Display for FormattedNotation {
    /// `FormattedNotation#to_s`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.notation_style {
            NotationStyle::E => write!(f, "{}{}{}", self.coefficient, self.exponent_separator, self.formatted_exponent()),
            _ => write!(f, "{} {} 10^{}", self.coefficient, self.times_symbol, self.formatted_exponent()),
        }
    }
}

/// Result of `NumberFormatter#formatted_number`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Formatted {
    Number(FormattedNumber),
    Notation(FormattedNotation),
}

impl std::fmt::Display for Formatted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Formatted::Number(n) => n.fmt(f),
            Formatted::Notation(n) => n.fmt(f),
        }
    }
}

/// `NumberRenderer` (constructor validation order: BaseNotation, Integer,
/// Fraction, Significant).
pub(crate) struct NumberRenderer<'a> {
    options: &'a FormatOptions,
    base_notation: BaseNotation,
    integer_format: IntegerFormat,
    fraction_format: FractionFormat,
    significant_format: Significant,
}

impl<'a> NumberRenderer<'a> {
    pub(crate) fn new(options: &'a FormatOptions) -> Result<Self, FormatterError> {
        let base_notation = BaseNotation::from_options(options)?;
        let base = base_notation.base;
        Ok(NumberRenderer {
            options,
            integer_format: IntegerFormat::new(options, base)?,
            fraction_format: FractionFormat::new(options, base)?,
            significant_format: Significant::new(options, base)?,
            base_notation,
        })
    }

    fn decimal_precision_for(&self, precision: i64) -> Option<i64> {
        if self.base_notation.base == 10 {
            Some(precision)
        } else {
            None
        }
    }

    /// `format` (the resolver always supplies `options.precision`).
    pub(crate) fn format(&mut self, source: &Source) -> Result<FormattedNumber, FormatterError> {
        let precision = self.options.precision;
        let parts = source.to_parts(self.decimal_precision_for(precision));
        self.format_parts(parts, precision)
    }

    pub(crate) fn format_parts(&mut self, parts: Parts, precision: i64) -> Result<FormattedNumber, FormatterError> {
        let mut parts = parts;
        if let Some(p) = self.decimal_precision_for(precision) {
            let truncated: String = parts.fraction_digits.chars().take(p.max(0) as usize).collect();
            parts = parts.with_digits(None, Some(&truncated));
        }
        let integer = self.integer_format.number_to_base(&parts.integer_digits);
        parts = parts.with_digits(Some(&integer), None);
        parts = self.fraction_format.apply_parts(&parts, precision)?;
        if self.significant_format.is_active() {
            parts = self.significant_format.apply_parts(&parts)?;
        }
        let integer_part = self.integer_format.format_groups(&parts.integer_digits)?;
        let fraction_part =
            if parts.is_fractional() { self.fraction_format.format_groups(&parts.fraction_digits) } else { String::new() };
        Ok(FormattedNumber {
            sign: parts.sign,
            integer_part,
            fraction_part,
            decimal_separator: self.fraction_format.decimal.clone(),
            base_notation: self.base_notation.clone(),
            number_sign: self.options.number_sign()?,
        })
    }
}

/// `NotationRenderer#render`.
pub(crate) fn render_notation(options: &FormatOptions, source: &Source) -> Result<FormattedNotation, FormatterError> {
    let precision = options.precision;
    let (style, coefficient, exponent) = match options.notation.as_deref() {
        Some("e") | Some("scientific") => {
            let (parts, exponent) = notation_parts(source);
            let style = if options.notation.as_deref() == Some("e") { NotationStyle::E } else { NotationStyle::Scientific };
            (style, NumberRenderer::new(options)?.format_parts(parts, precision)?, exponent)
        }
        _ => {
            let (mut parts, mut exponent) = notation_parts(source);
            if !source.zero {
                (parts, exponent) = engineering_coefficient_parts(parts, exponent);
            }
            let p = engineering_precision(options, source, &parts)?;
            (NotationStyle::Engineering, NumberRenderer::new(options)?.format_parts(parts, p)?, exponent)
        }
    };
    Ok(FormattedNotation {
        coefficient,
        notation_style: style,
        exponent,
        times_symbol: options.times.clone(),
        exponent_separator: options.exponent_separator.clone(),
        exponent_sign: options.exponent_sign.clone(),
    })
}

fn notation_parts(source: &Source) -> (Parts, i64) {
    if source.zero {
        return (source.to_parts(None), 0);
    }
    let parts = source.to_parts(None);
    let (digits, exponent) = if parts.is_integer_zero() {
        let index = parts.fraction_digits.find(|c: char| ('1'..='9').contains(&c)).unwrap_or(0);
        (parts.fraction_digits[index..].to_string(), -(index as i64 + 1))
    } else {
        let digits = format!("{}{}", parts.integer_digits, parts.fraction_digits);
        let index = digits.find(|c: char| ('1'..='9').contains(&c)).unwrap_or(0);
        (digits[index..].to_string(), parts.integer_digits.len() as i64 - index as i64 - 1)
    };
    (Parts::new(parts.sign, &digits[..1], &digits[1..]), exponent)
}

fn engineering_coefficient_parts(coefficient: Parts, exponent: i64) -> (Parts, i64) {
    let index = exponent.rem_euclid(3);
    let new_exponent = exponent - index;
    let digits = format!("{}{}", coefficient.integer_digits, coefficient.fraction_digits);
    let integer_length = (index + 1) as usize;
    let mut integer: String = digits.chars().take(integer_length).collect();
    while integer.len() < integer_length {
        integer.push('0');
    }
    let fraction: String = digits.chars().skip(integer_length).collect();
    (Parts::new(coefficient.sign, &integer, &fraction), new_exponent)
}

fn engineering_precision(options: &FormatOptions, source: &Source, coefficient: &Parts) -> Result<i64, FormatterError> {
    let precision = options.precision;
    if options.explicit_precision && precision > 0 {
        return Ok(precision);
    }
    if source.zero {
        return Ok(source.notation_precision());
    }
    let integer_length = coefficient.integer_digits.len() as i64;
    let budget = options.significant()?.max(options.digit_count()?);
    let source_significant = source.significant_digit_count() as i64;
    if budget <= 0 {
        return Ok((source_significant - integer_length).max(0));
    }
    let mut fraction_budget = (budget - integer_length).max(0);
    if source_significant > budget {
        fraction_budget += 1;
    }
    Ok(fraction_budget)
}
