//! `FormatOptions`, `SymbolResolver`, `PrecisionResolver` and locale lookup.

use super::locales::LOCALES;
use super::source::Source;
use super::value::{kernel_integer_base10, RubyValue, SymbolMap};
use super::{ConfigErrorKind, FormatterError};

/// `NumberFormatter#supported_locale`: String/Symbol present in
/// `SupportedLocales::LOCALES`, else `:en`; never raises.
pub(crate) fn supported_locale(locale: &RubyValue) -> &'static str {
    if let Some(name) = locale.string_or_symbol() {
        if let Some((key, _, _)) = LOCALES.iter().find(|(k, _, _)| *k == name) {
            return key;
        }
    }
    "en"
}

fn locale_symbols(locale: &str) -> SymbolMap {
    let (_, decimal, group) = LOCALES.iter().find(|(k, _, _)| *k == locale).copied().unwrap_or(("en", ".", ","));
    SymbolMap::from_pairs(&[("decimal", RubyValue::str(decimal)), ("group", RubyValue::str(group))])
}

/// `SymbolResolver::LOCALIZE_NUMBER_REGEX` (unanchored, leftmost match):
/// `(?<group>[^#])?(?<groupdigits>#+0)(?<decimal>.)(?<fractdigits>#+)(?<fractgroup>[^#])?`
fn localize_number_symbols(template: &str) -> SymbolMap {
    let c: Vec<char> = template.chars().collect();
    // Match the mandatory tail starting at `at`; returns (groupdigits, decimal, fractdigits, fractgroup).
    let tail = |at: usize| -> Option<(usize, char, usize, Option<char>)> {
        let mut i = at;
        while i < c.len() && c[i] == '#' {
            i += 1;
        }
        if i == at || i >= c.len() || c[i] != '0' {
            return None;
        }
        let groupdigits = i - at + 1;
        i += 1;
        let decimal = *c.get(i).filter(|ch| **ch != '\n')?;
        i += 1;
        let f = i;
        while i < c.len() && c[i] == '#' {
            i += 1;
        }
        if i == f {
            return None;
        }
        Some((groupdigits, decimal, i - f, c.get(i).copied()))
    };
    for (start, ch) in c.iter().copied().enumerate() {
        let attempt = if ch != '#' {
            tail(start + 1).map(|t| (Some(ch), t)).or_else(|| tail(start).map(|t| (None, t)))
        } else {
            tail(start).map(|t| (None, t))
        };
        if let Some((group, (gd, decimal, fd, fg))) = attempt {
            let nbsp = |ch: Option<char>| -> String {
                match ch {
                    Some(' ') => "\u{00A0}".into(),
                    Some(x) => x.to_string(),
                    None => String::new(),
                }
            };
            return SymbolMap::from_pairs(&[
                ("decimal", RubyValue::Str(decimal.to_string())),
                ("group_digits", RubyValue::Int(gd as i64)),
                ("fraction_group_digits", RubyValue::Int(fd as i64)),
                ("group", RubyValue::Str(nbsp(group))),
                ("fraction_group", RubyValue::Str(nbsp(fg))),
            ]);
        }
    }
    SymbolMap::new()
}

/// `SymbolResolver#resolve` merged with the per-call `format`
/// (`NumberFormatter#symbols_for`). Inputs are already validated.
pub(crate) fn resolve_symbols(
    locale: &str,
    localizer_symbols: &SymbolMap,
    localize_number: Option<&str>,
    format: &SymbolMap,
) -> SymbolMap {
    let template = localize_number.map(localize_number_symbols).unwrap_or_default();
    locale_symbols(locale).merged(localizer_symbols).merged(&template).merged(format)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HexCapital {
    None,
    True,
    NumbersOnly,
}

/// `FormatOptions`.
#[derive(Clone, Debug)]
pub(crate) struct FormatOptions {
    pub(crate) symbols: SymbolMap,
    pub(crate) notation: Option<String>,
    pub(crate) exponent_separator: String,
    pub(crate) times: String,
    pub(crate) precision: i64,
    pub(crate) explicit_precision: bool,
    pub(crate) exponent_sign: Option<String>,
}

fn invalid(key: &str, value: &RubyValue, expected: Option<&str>) -> FormatterError {
    FormatterError::Configuration {
        kind: ConfigErrorKind::InvalidFormatterOption,
        option: Some(key.to_string()),
        value: Some(value.inspect()),
        supported: expected.map(str::to_string),
    }
}

/// `coerce_integer`: `Err(())` is the Ruby `yield` branch.
fn coerce_integer(value: &RubyValue) -> Result<i64, Option<FormatterError>> {
    match value {
        RubyValue::Int(i) => Ok(*i),
        RubyValue::Bool(_) | RubyValue::Float(_) => Err(None),
        other => match kernel_integer_base10(&other.to_s()) {
            Ok(Some(i)) => Ok(i),
            Ok(None) => Err(Some(FormatterError::ResourceLimit { what: "integer option" })),
            Err(()) => Err(None),
        },
    }
}

const NON_NEGATIVE: &str = "a non-negative integer";

impl FormatOptions {
    /// `FormatOptions.new(source, symbols:, precision:, precision_resolver:)`
    /// with the source `PrecisionResolver` always supplied.
    pub(crate) fn new(source: &Source, symbols: SymbolMap, precision: &RubyValue) -> Result<Self, FormatterError> {
        let mut o = FormatOptions {
            symbols,
            notation: None,
            exponent_separator: "e".into(),
            times: "\u{d7}".into(),
            precision: 0,
            explicit_precision: false,
            exponent_sign: None,
        };
        o.notation = o.symbol_option("notation")?;
        o.exponent_separator = o.symbol_option("e")?.unwrap_or_else(|| "e".into());
        o.times = o.symbol_option("times")?.unwrap_or_else(|| "\u{d7}".into());
        o.precision = o.resolve_precision(source, precision)?;
        o.exponent_sign = o.symbol_option("exponent_sign")?;
        if o.symbols.has("padding_digits") && o.symbols.has("padding_group_digits") {
            return Err(FormatterError::Configuration {
                kind: ConfigErrorKind::ConflictingFormatterOptions,
                option: None,
                value: None,
                supported: Some("[:padding_digits, :padding_group_digits]".into()),
            });
        }
        Ok(o)
    }

    fn resolve_precision(&mut self, source: &Source, precision: &RubyValue) -> Result<i64, FormatterError> {
        let effective = if precision.is_nil() { self.symbols.get("precision") } else { precision.clone() };
        let explicit = if effective.is_nil() {
            None
        } else {
            let v = match coerce_integer(&effective) {
                Ok(v) => v,
                Err(Some(e)) => return Err(e),
                Err(None) => return Err(invalid("precision", &effective, None)),
            };
            if v < 0 {
                return Err(invalid("precision", &effective, Some(NON_NEGATIVE)));
            }
            Some(v)
        };
        self.explicit_precision = explicit.is_some();
        // Ruby evaluates every resolver keyword before `resolve` runs.
        let base = self.base();
        let significant = self.significant()?;
        let digit_count = self.digit_count()?;
        let notation_supported = self.notation_supported();
        Ok(resolve_precision(source, explicit, &base, significant, notation_supported, digit_count))
    }

    /// `base`: numeric forms coerced, anything else left raw.
    pub(crate) fn base(&self) -> RubyValue {
        let value = self.symbols.get("base");
        if value.is_nil() {
            return RubyValue::Int(10);
        }
        match coerce_integer(&value) {
            Ok(i) => RubyValue::Int(i),
            Err(_) => value,
        }
    }
    pub(crate) fn base_prefix(&self) -> Result<String, FormatterError> {
        Ok(self.separator_option("base_prefix", RubyValue::Nil)?.to_s())
    }
    pub(crate) fn has_base_prefix(&self) -> bool {
        self.symbols.has("base_prefix")
    }
    pub(crate) fn base_postfix(&self) -> Result<RubyValue, FormatterError> {
        self.separator_option("base_postfix", RubyValue::Nil)
    }
    pub(crate) fn has_base_postfix(&self) -> bool {
        self.symbols.has("base_postfix")
    }
    pub(crate) fn decimal(&self) -> Result<RubyValue, FormatterError> {
        self.separator_option("decimal", RubyValue::str("."))
    }
    pub(crate) fn digit_count(&self) -> Result<i64, FormatterError> {
        Ok(self.integer_option("digit_count")?.unwrap_or(0))
    }
    pub(crate) fn fraction_group(&self) -> Result<String, FormatterError> {
        Ok(self.separator_option("fraction_group", RubyValue::Nil)?.to_s())
    }
    pub(crate) fn fraction_group_digits(&self) -> Result<Option<i64>, FormatterError> {
        self.integer_option("fraction_group_digits")
    }
    pub(crate) fn group(&self) -> Result<String, FormatterError> {
        let v = self.separator_option("group", RubyValue::str(","))?;
        Ok(if v.truthy() { v.to_s() } else { ",".into() })
    }
    pub(crate) fn group_digits(&self) -> Result<i64, FormatterError> {
        Ok(self.integer_option("group_digits")?.unwrap_or(3))
    }
    pub(crate) fn hex_capital(&self) -> Result<HexCapital, FormatterError> {
        let value = self.symbols.get("hex_capital");
        match &value {
            RubyValue::Nil => Ok(HexCapital::None),
            RubyValue::Bool(_) | RubyValue::Str(_) | RubyValue::Sym(_) => Ok(match value.to_s().as_str() {
                "true" => HexCapital::True,
                "numbers_only" => HexCapital::NumbersOnly,
                _ => HexCapital::None,
            }),
            _ => Err(invalid("hex_capital", &value, None)),
        }
    }
    pub(crate) fn number_sign(&self) -> Result<Option<String>, FormatterError> {
        self.symbol_option("number_sign")
    }
    pub(crate) fn padding(&self) -> Result<String, FormatterError> {
        let v = self.separator_option("padding", RubyValue::str("0"))?.to_s();
        Ok(v.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "0".into()))
    }
    pub(crate) fn padding_digits(&self) -> Result<i64, FormatterError> {
        Ok(self.integer_option("padding_digits")?.unwrap_or(0))
    }
    pub(crate) fn padding_group_digits(&self) -> Result<i64, FormatterError> {
        Ok(self.integer_option("padding_group_digits")?.unwrap_or(0))
    }
    pub(crate) fn notation_supported(&self) -> bool {
        matches!(self.notation.as_deref(), Some("e" | "scientific" | "engineering"))
    }
    pub(crate) fn significant(&self) -> Result<i64, FormatterError> {
        Ok(self.integer_option("significant")?.unwrap_or(0))
    }

    fn integer_option(&self, key: &str) -> Result<Option<i64>, FormatterError> {
        let value = self.symbols.get(key);
        if value.is_nil() {
            return Ok(None);
        }
        let i = match coerce_integer(&value) {
            Ok(i) => i,
            Err(Some(e)) => return Err(e),
            Err(None) => return Err(invalid(key, &value, None)),
        };
        if i < 0 {
            return Err(invalid(key, &value, Some(NON_NEGATIVE)));
        }
        Ok(Some(i))
    }
    fn separator_option(&self, key: &str, default: RubyValue) -> Result<RubyValue, FormatterError> {
        let value = self.symbols.fetch(key, default);
        if value.is_bool() {
            return Err(invalid(key, &value, None));
        }
        Ok(value)
    }
    fn symbol_option(&self, key: &str) -> Result<Option<String>, FormatterError> {
        let value = self.symbols.get(key);
        if value.is_nil() {
            return Ok(None);
        }
        match value.string_or_symbol() {
            Some(s) => Ok(Some(s.to_string())),
            None => Err(invalid(key, &value, None)),
        }
    }
}

/// `PrecisionResolver#resolve`.
pub(crate) fn resolve_precision(
    source: &Source,
    precision: Option<i64>,
    base: &RubyValue,
    significant: i64,
    notation_supported: bool,
    digit_count: i64,
) -> i64 {
    if let Some(p) = precision {
        return p;
    }
    if let Some(p) = significant_base_precision(source, base, significant) {
        return p;
    }
    if notation_supported {
        let budget = significant.max(digit_count);
        if budget > 0 {
            return (budget - 1).max(source.notation_precision());
        }
        return source.notation_precision();
    }
    source.decimal_precision()
}

fn significant_base_precision(source: &Source, base: &RubyValue, significant: i64) -> Option<i64> {
    let target = match base {
        RubyValue::Int(b @ (2 | 8 | 16)) => *b as u32,
        _ => return None,
    };
    if significant == 0 {
        return None;
    }
    if !source.is_fractional() {
        return Some(0);
    }
    let source_significant = source.significant_digit_count() as i64;
    let effective = significant.min(source_significant);
    let mut target_precision = (effective - source.target_base_integer_length(target) as i64).max(0);
    if source_significant > effective {
        target_precision += 1;
    }
    Some(target_precision)
}
