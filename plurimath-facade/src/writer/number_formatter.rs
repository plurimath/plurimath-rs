//! Native port of the Plurimath Ruby number formatter
//! (Ruby 0a1df68a5903d43da13b1e1ef85b6c0523feb1b4: `number_formatter.rb`,
//! `formatter/standard.rb`, `formatter/supported_locales.rb`,
//! `formatter/numbers/*.rb`, and the formatter branches of
//! `math/number.rb`).
//!
//! Exact string/digit arithmetic only (no floats). Default/unformatted Number
//! rendering is NOT handled here: callers only enter this module when a
//! formatter is configured (`Number#number_formatter(options)` truthy).
//!
//! Internal entry point: [`format_number_for_writer`]. The parent owns the
//! public `formatterOptions` request adapter and maps it onto
//! [`FormatterProfile`] / [`HashArg`] / [`RubyValue`].
#![allow(dead_code)]

#[path = "number_formatter/digits.rs"]
mod digits;
use crate::locale_data as locales;
#[path = "number_formatter/options.rs"]
mod options;
#[path = "number_formatter/render.rs"]
mod render;
#[path = "number_formatter/source.rs"]
mod source;
#[path = "number_formatter/transforms.rs"]
mod transforms;
#[path = "number_formatter/value.rs"]
mod value;
#[path = "number_formatter/writers.rs"]
mod writers;

#[cfg(test)]
#[path = "number_formatter/tests.rs"]
mod tests;

#[allow(unused_imports)]
pub(crate) use render::{Formatted, FormattedNotation, FormattedNumber, NotationStyle};
pub(crate) use value::{HashArg, RubyValue, SymbolMap};
#[allow(unused_imports)]
pub(crate) use writers::{render_for_writer, FmtXml, TargetWriter, WriterOutput};

use options::{resolve_symbols, supported_locale, FormatOptions};
use source::Source;

/// Rust-side guard on digit expansion (exponent shift, padding, precision).
/// Ruby has no such cap; exceeding it is a documented native refusal
/// (`FormatterError::ResourceLimit`), never a silently different answer.
pub(crate) const MAX_EXPANSION: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigErrorKind {
    /// `ConfigurationError.new(:invalid_formatter_option, option:, value:, supported:)`
    InvalidFormatterOption,
    /// `ConfigurationError.new(:conflicting_formatter_options, supported:)`
    ConflictingFormatterOptions,
}

/// Ruby error classes raised by the formatter path, kept explicit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FormatterError {
    /// `Plurimath::ConfigurationError`; `value` is the Ruby `inspect` text.
    Configuration { kind: ConfigErrorKind, option: Option<String>, value: Option<String>, supported: Option<String> },
    /// `Plurimath::Errors::InvalidNumber` (value = raw number text).
    InvalidNumber { value: String },
    /// `Plurimath::Errors::UnsupportedBase` (base = Ruby `inspect`).
    UnsupportedBase { base: String },
    /// Ruby core `ArgumentError` reached through source (`BigDecimal()`,
    /// `String#to_i` radix).
    RubyArgumentError { message: String },
    /// Ruby core `TypeError` reached through source (`String#to_i(String)`).
    RubyTypeError { message: String },
    /// Ruby core `NoMethodError` (`Number#format_value_with_options` on a
    /// non-Hash `options[:format]` together with a Number base).
    RubyNoMethodError { message: String },
    /// Native refusal (not a Ruby class): see [`MAX_EXPANSION`].
    ResourceLimit { what: &'static str },
}

impl FormatterError {
    /// Ruby class name of the source error.
    pub(crate) fn ruby_class(&self) -> &'static str {
        match self {
            FormatterError::Configuration { .. } => "Plurimath::ConfigurationError",
            FormatterError::InvalidNumber { .. } => "Plurimath::Errors::InvalidNumber",
            FormatterError::UnsupportedBase { .. } => "Plurimath::Errors::UnsupportedBase",
            FormatterError::RubyArgumentError { .. } => "ArgumentError",
            FormatterError::RubyTypeError { .. } => "TypeError",
            FormatterError::RubyNoMethodError { .. } => "NoMethodError",
            FormatterError::ResourceLimit { .. } => "native:ResourceLimit",
        }
    }
}

/// Which Ruby formatter object the caller configured.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FormatterProfile {
    /// `Plurimath::Formatter::Standard.new(locale:, string_format:, options:, precision:)`
    /// (the formatter66 `number_formatter` call profile; defaults
    /// `locale: "en"`, `string_format: nil`, `options: {}`, `precision: nil`).
    Standard { locale: RubyValue, string_format: RubyValue, options: HashArg, precision: RubyValue },
    /// `Plurimath::NumberFormatter.new(locale, localize_number:, localizer_symbols:, precision:)`
    /// (defaults `"en"`, `nil`, `{}`, `nil`).
    Base { locale: RubyValue, localize_number: RubyValue, localizer_symbols: HashArg, precision: RubyValue },
}

/// `Formatter::Standard::DEFAULT_OPTIONS` keys applied by
/// `apply_default_symbols` (precision/digit_count are NOT applied).
fn standard_defaults() -> [(&'static str, RubyValue); 12] {
    [
        ("fraction_group_digits", RubyValue::Int(3)),
        ("fraction_group", RubyValue::str("'")),
        ("exponent_sign", RubyValue::Nil),
        ("group_digits", RubyValue::Int(3)),
        ("padding", RubyValue::str("0")),
        ("number_sign", RubyValue::Nil),
        ("significant", RubyValue::Int(0)),
        ("notation", RubyValue::sym("basic")),
        ("decimal", RubyValue::str(".")),
        ("group", RubyValue::str(",")),
        ("times", RubyValue::str("x")),
        ("e", RubyValue::str("e")),
    ]
}

/// A constructed Ruby `NumberFormatter` (or `Standard`) instance.
#[derive(Clone, Debug)]
pub(crate) struct NumberFormatter {
    locale: &'static str,
    localize_number: RubyValue,
    localizer_symbols: HashArg,
    precision: RubyValue,
}

impl NumberFormatter {
    /// Construct as Ruby would (only `Standard` can raise at construction).
    pub(crate) fn new(profile: &FormatterProfile) -> Result<Self, FormatterError> {
        match profile {
            FormatterProfile::Base { locale, localize_number, localizer_symbols, precision } => Ok(NumberFormatter {
                locale: supported_locale(locale),
                localize_number: localize_number.clone(),
                localizer_symbols: localizer_symbols.clone(),
                precision: precision.clone(),
            }),
            FormatterProfile::Standard { locale, string_format, options, precision } => {
                let mut symbols = match options {
                    HashArg::Nil => SymbolMap::new(),
                    HashArg::Hash(h) => h.clone(),
                    HashArg::NotHash(v) => {
                        return Err(FormatterError::Configuration {
                            kind: ConfigErrorKind::InvalidFormatterOption,
                            option: Some("options".into()),
                            value: Some(v.inspect()),
                            supported: Some("a Hash".into()),
                        })
                    }
                };
                for (k, v) in standard_defaults() {
                    if !symbols.has(k) {
                        symbols.set(k, v);
                    }
                }
                Ok(NumberFormatter {
                    locale: supported_locale(locale),
                    localize_number: string_format.clone(),
                    localizer_symbols: HashArg::Hash(symbols),
                    precision: precision.clone(),
                })
            }
        }
    }

    /// `NumberFormatter#formatted_number(number_string, format:)` with the
    /// instance locale and precision.
    pub(crate) fn formatted_number(&self, number: &str, format: &HashArg) -> Result<Formatted, FormatterError> {
        let format = match format {
            HashArg::Nil => SymbolMap::new(),
            HashArg::Hash(h) => h.clone(),
            HashArg::NotHash(v) => {
                return Err(FormatterError::Configuration {
                    kind: ConfigErrorKind::InvalidFormatterOption,
                    option: Some("format".into()),
                    value: Some(v.inspect()),
                    supported: Some("a Hash of formatter options".into()),
                })
            }
        };
        let source = Source::new(number, &format.fetch("base", RubyValue::Int(10)))?;
        let symbols = self.symbols_for(&format)?;
        let options = FormatOptions::new(&source, symbols, &self.precision)?;
        if options.notation_supported() {
            return Ok(Formatted::Notation(render::render_notation(&options, &source)?));
        }
        Ok(Formatted::Number(render::NumberRenderer::new(&options)?.format(&source)?))
    }

    /// `NumberFormatter#localized_number` (`formatted_number(...).to_s`).
    pub(crate) fn localized_number(&self, number: &str, format: &HashArg) -> Result<String, FormatterError> {
        Ok(self.formatted_number(number, format)?.to_string())
    }

    /// `Number#format_value_with_options` + `NumberFormatter#format_number`:
    /// `writer_format` is `options[:format]` (`Nil` when absent/false),
    /// `number_base` is the Number model's `base` attribute.
    pub(crate) fn format_number(
        &self,
        value: &str,
        number_base: Option<i64>,
        writer_format: &HashArg,
    ) -> Result<Formatted, FormatterError> {
        let format = match (writer_format, number_base) {
            (HashArg::NotHash(_), Some(_)) => {
                return Err(FormatterError::RubyNoMethodError { message: "undefined method 'key?'".into() })
            }
            (HashArg::Nil, Some(b)) => HashArg::Hash(SymbolMap::from_pairs(&[("base", RubyValue::Int(b))])),
            (HashArg::Hash(h), Some(b)) if !h.has("base") => {
                let mut h = h.clone();
                h.set("base", RubyValue::Int(b));
                HashArg::Hash(h)
            }
            (HashArg::Nil, None) => HashArg::empty(),
            (other, _) => other.clone(),
        };
        self.formatted_number(value, &format)
    }

    fn symbols_for(&self, format: &SymbolMap) -> Result<SymbolMap, FormatterError> {
        let localizer = match &self.localizer_symbols {
            HashArg::Nil => SymbolMap::new(),
            HashArg::Hash(h) => h.clone(),
            HashArg::NotHash(v) => {
                return Err(FormatterError::Configuration {
                    kind: ConfigErrorKind::InvalidFormatterOption,
                    option: Some("localizer_symbols".into()),
                    value: Some(v.inspect()),
                    supported: Some("a Hash".into()),
                })
            }
        };
        let template = match &self.localize_number {
            RubyValue::Nil => None,
            RubyValue::Str(s) => Some(s.as_str()),
            other => {
                return Err(FormatterError::Configuration {
                    kind: ConfigErrorKind::InvalidFormatterOption,
                    option: Some("localize_number".into()),
                    value: Some(other.inspect()),
                    supported: Some("a String".into()),
                })
            }
        };
        Ok(resolve_symbols(self.locale, &localizer, template, format))
    }
}

/// INTERNAL helper for the parent writer adapter: one Number, one writer.
///
/// * `source_text` — `number.value.to_s` (textual magnitude as authored).
/// * `number_base` — the Number model's `base` attribute.
/// * `writer` — which Number writer method is being rendered.
/// * `profile` — the configured formatter (`formatterOptions`).
/// * `writer_format` — `options[:format]` of the writer call (`HashArg::Nil` if absent).
///
/// Returns text for text writers/`PlainText`, or one structural XML node
/// for MathML/OMML; errors carry the Ruby error class explicitly.
pub(crate) fn format_number_for_writer(
    source_text: &str,
    number_base: Option<i64>,
    writer: TargetWriter,
    profile: &FormatterProfile,
    writer_format: &HashArg,
) -> Result<WriterOutput, FormatterError> {
    let formatter = NumberFormatter::new(profile)?;
    let result = formatter.format_number(source_text, number_base, writer_format)?;
    Ok(render_for_writer(&result, writer))
}
