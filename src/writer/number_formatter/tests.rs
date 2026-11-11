//! Source-justified tests on numbers outside the formatter66 corpus. Each
//! expectation is derived by hand from the cited Ruby method, not from this
//! implementation.

use super::*;

fn base_profile(precision: RubyValue) -> FormatterProfile {
    FormatterProfile::Base {
        locale: RubyValue::str("en"),
        localize_number: RubyValue::Nil,
        localizer_symbols: HashArg::empty(),
        precision,
    }
}

fn map(pairs: &[(&str, RubyValue)]) -> HashArg {
    HashArg::Hash(SymbolMap::from_pairs(pairs))
}

fn out(profile: &FormatterProfile, n: &str, w: TargetWriter, format: &[(&str, RubyValue)]) -> String {
    match format_number_for_writer(n, None, w, profile, &map(format)) {
        Ok(WriterOutput::Text(t)) => t,
        Ok(WriterOutput::Xml(x)) => x.to_compact(),
        Err(e) => format!("ERR {}", e.ruby_class()),
    }
}

fn s(n: &str, format: &[(&str, RubyValue)]) -> String {
    out(&base_profile(RubyValue::Nil), n, TargetWriter::PlainText, format)
}

use RubyValue::{Int, Nil};
fn st(v: &str) -> RubyValue {
    RubyValue::str(v)
}

#[test]
fn plain_decimal_keeps_scale_and_groups_integer() {
    // PrecisionResolver -> Source#decimal_precision (3); Integer#format_groups.
    assert_eq!(s("1234567.891", &[]), "1,234,567.891");
    assert_eq!(s("-0.5e3", &[]), "-500");
    assert_eq!(s("12e-3", &[]), "0.012");
}

#[test]
fn precision_truncates_never_rounds() {
    // Source#apply_precision slices fraction[0...size].
    let p = base_profile(Int(2));
    assert_eq!(out(&p, "2.999", TargetWriter::PlainText, &[]), "2.99");
    assert_eq!(out(&p, "7", TargetWriter::PlainText, &[]), "7.00");
}

#[test]
fn scientific_and_engineering_notation() {
    // notation_parts: "0.000123" -> digits "123", exponent -4; precision 2.
    assert_eq!(s("-0.000123", &[("notation", st("scientific"))]), "-1.23 \u{d7} 10^-4");
    // engineering: exponent 5 -> 3, integer "123", budget 6-3.
    assert_eq!(s("123456", &[("notation", st("engineering"))]), "123.456 \u{d7} 10^3");
    // engineering negative exponent: -4 % 3 == 2 -> 10^-6, "12".ljust(3,"0").
    assert_eq!(s("0.00012", &[("notation", st("engineering"))]), "120 \u{d7} 10^-6");
    assert_eq!(
        s("12345", &[("notation", st("e")), ("e", st("E")), ("exponent_sign", st("plus"))]),
        "1.2345E+4"
    );
}

#[test]
fn significant_carry_drops_decimal_point() {
    // Significant#increment_fractional overflows "" and the point is dropped.
    assert_eq!(s("9.996", &[("significant", Int(3))]), "10");
    assert_eq!(s("123456", &[("significant", Int(2))]), "120,000");
}

#[test]
fn digit_count_rounds_into_new_integer_digit() {
    // Fraction#round_base_string + round_integer pops one fraction digit.
    assert_eq!(s("99.996", &[("digit_count", Int(4))]), "100.0");
    assert_eq!(s("1.5", &[("digit_count", Int(4))]), "1.500");
}

#[test]
fn hex_with_fraction_and_semantic_writers() {
    // change_base("5", 1): 0.5*16 = 8; hex_capital true upcases digits only.
    let p = base_profile(Nil);
    let f = [("base", Int(16)), ("hex_capital", RubyValue::Bool(true))];
    assert_eq!(out(&p, "-255.5", TargetWriter::PlainText, &f), "-0xFF.8");
    assert_eq!(out(&p, "-255.5", TargetWriter::AsciiMath, &f), "-FF.8_(16)");
    assert_eq!(out(&p, "-255.5", TargetWriter::Latex, &f), "-\\mathrm{FF.8}_{16}");
    assert_eq!(out(&p, "-255.5", TargetWriter::Html, &f), "-FF.8<sub>16</sub>");
    assert_eq!(out(&p, "-255.5", TargetWriter::MathMl, &f), "<mrow><mo>-</mo><msub><mn>FF.8</mn><mn>16</mn></msub></mrow>");
}

#[test]
fn literal_prefix_disables_semantic_base() {
    let f = [("base", Int(16)), ("hex_capital", st("numbers_only")), ("base_prefix", st("#"))];
    let p = base_profile(Nil);
    // Integer#format_groups still groups target-base digits (default 3).
    assert_eq!(out(&p, "48879", TargetWriter::AsciiMath, &f), "#B,EEF");
    let f4 = [f[0].clone(), f[1].clone(), f[2].clone(), ("group_digits", Int(4))];
    assert_eq!(out(&p, "48879", TargetWriter::MathMl, &f4), "<mn>#BEEF</mn>");
}

#[test]
fn binary_padding_groups_and_exact_fraction() {
    let f = [("base", Int(2)), ("padding_group_digits", Int(4)), ("group_digits", Int(4)), ("group", st(" "))];
    assert_eq!(s("37", &f), "0b0010 0101");
    // 0.1 in base 2, 8 digits by exact Rational stepping: 00011001.
    let p = base_profile(Int(8));
    assert_eq!(out(&p, "0.1", TargetWriter::PlainText, &[("base", Int(2))]), "0b0.00011001");
}

#[test]
fn bignum_base_conversion_is_exact() {
    assert_eq!(s("18446744073709551616", &[("base", Int(16)), ("group_digits", Int(0))]), "0x10000000000000000");
    assert_eq!(s("1e30", &[("base", Int(8)), ("group_digits", Int(0))]), "0o1447626234640431647336510000000000");
}

#[test]
fn number_base_attribute_reads_alphanumeric_digits() {
    // Number#format_value_with_options merges base:; Source uses to_i(16).
    let p = base_profile(Nil);
    let r = format_number_for_writer("ff", Some(16), TargetWriter::AsciiMath, &p, &HashArg::Nil);
    assert_eq!(r, Ok(WriterOutput::Text("ff_(16)".into())));
}

#[test]
fn localize_number_template_and_locales() {
    let p = FormatterProfile::Base {
        locale: RubyValue::str("en"),
        localize_number: RubyValue::str("#,##0.### #"),
        localizer_symbols: HashArg::Nil,
        precision: Nil,
    };
    assert_eq!(out(&p, "1234.56789", TargetWriter::PlainText, &[]), "1,234.567\u{a0}89");
    let de = FormatterProfile::Base {
        locale: RubyValue::sym("de"),
        localize_number: Nil,
        localizer_symbols: HashArg::Nil,
        precision: Nil,
    };
    assert_eq!(out(&de, "1234567.5", TargetWriter::PlainText, &[]), "1.234.567,5");
    // Explicit nil decimal renders without a separator.
    assert_eq!(s("1.5", &[("decimal", Nil)]), "15");
}

#[test]
fn standard_defaults_override_locale() {
    let p = FormatterProfile::Standard {
        locale: RubyValue::str("fr"),
        string_format: Nil,
        options: HashArg::empty(),
        precision: Nil,
    };
    assert_eq!(out(&p, "9876.54321", TargetWriter::PlainText, &[]), "9,876.543'21");
    assert_eq!(
        out(&p, "0.5", TargetWriter::Omml, &[("notation", st("scientific"))]),
        "<m:sSup><m:sSupPr><m:ctrlPr><w:rPr><w:rFonts w:ascii=\"Cambria Math\" w:hAnsi=\"Cambria Math\"/><w:i/></w:rPr></m:ctrlPr></m:sSupPr><m:e><m:r><m:t>5</m:t></m:r><m:r><m:t> x </m:t></m:r><m:r><m:t>10</m:t></m:r></m:e><m:sup><m:r><m:t>-1</m:t></m:r></m:sup></m:sSup>"
    );
}

#[test]
fn explicit_ruby_error_classes() {
    assert_eq!(s("12a", &[]), "ERR Plurimath::Errors::InvalidNumber");
    assert_eq!(s("12", &[("base", Int(3))]), "ERR Plurimath::Errors::UnsupportedBase");
    assert_eq!(s("1", &[("padding_digits", Int(2)), ("padding_group_digits", Int(2))]), "ERR Plurimath::ConfigurationError");
    assert_eq!(s("1", &[("significant", Int(-1))]), "ERR Plurimath::ConfigurationError");
    assert_eq!(s("1", &[("precision", st("x"))]), "ERR Plurimath::ConfigurationError");
    assert_eq!(s("1", &[("group", RubyValue::Bool(false))]), "ERR Plurimath::ConfigurationError");
    let bad = FormatterProfile::Standard {
        locale: Nil,
        string_format: Nil,
        options: HashArg::NotHash(Int(1)),
        precision: Nil,
    };
    assert_eq!(out(&bad, "1", TargetWriter::PlainText, &[]), "ERR Plurimath::ConfigurationError");
    let sym_template = FormatterProfile::Base {
        locale: Nil,
        localize_number: RubyValue::sym("#,##0.### #"),
        localizer_symbols: HashArg::Nil,
        precision: Nil,
    };
    assert_eq!(out(&sym_template, "1", TargetWriter::PlainText, &[]), "ERR Plurimath::ConfigurationError");
}

#[test]
fn numeric_string_options_are_coerced() {
    // coerce_integer: Integer("4", 10); base "16" normalizes to 16.
    assert_eq!(s("1234567", &[("group_digits", st("4")), ("group", st("_"))]), "123_4567");
    assert_eq!(s("255", &[("base", st("16"))]), "0xff");
}

fn with_base(n: &str, base: i64, w: TargetWriter) -> String {
    match format_number_for_writer(n, Some(base), w, &base_profile(Nil), &HashArg::Nil) {
        Ok(WriterOutput::Text(t)) => t,
        Ok(WriterOutput::Xml(x)) => x.to_compact(),
        Err(e) => format!("ERR {}", e.ruby_class()),
    }
}

#[test]
fn non_decimal_source_uses_string_to_i_prefix_scan() {
    // Source#parse_decimal: "1g".to_i(16) == 1; "0x1f".to_i(16) == 31.
    assert_eq!(with_base("1g", 16, TargetWriter::AsciiMath), "1_(16)");
    assert_eq!(with_base("-0x1f", 16, TargetWriter::AsciiMath), "-1f_(16)");
    // Digits-only text stays a decimal magnitude even under base 2 (and the
    // default group_digits 3 still applies).
    assert_eq!(with_base("10", 2, TargetWriter::PlainText), "0b1,010");
    // Radix outside String#to_i range fails in Source before BaseNotation.
    assert_eq!(with_base("zz", 37, TargetWriter::PlainText), "ERR ArgumentError");
    // String#to_i(0) auto-detects and never raises, so base 0 reaches
    // BaseNotation.validate! (UnsupportedBase), unlike radix 1 (ArgumentError).
    assert_eq!(s("ff", &[("base", Int(0))]), "ERR Plurimath::Errors::UnsupportedBase");
    assert_eq!(s("ff", &[("base", Int(1))]), "ERR ArgumentError");
    // A String base is non-decimal for Source; "ff".to_i("16") is a TypeError.
    assert_eq!(s("ff", &[("base", st("16"))]), "ERR TypeError");
    // decimal_input? accepts "1.2.3" but BigDecimal() rejects it.
    assert_eq!(s("1.2.3", &[("base", Int(16))]), "ERR ArgumentError");
}

#[test]
fn explicit_zero_precision_and_notation() {
    let p = base_profile(Int(0));
    // engineering_precision: explicit 0 falls through to inference (6 - 3).
    assert_eq!(out(&p, "123456", TargetWriter::PlainText, &[("notation", st("engineering"))]), "123.456 \u{d7} 10^3");
    // e notation honours the resolved precision 0 for the coefficient.
    assert_eq!(out(&p, "123456", TargetWriter::PlainText, &[("notation", st("e"))]), "1e5");
    // MathML for scientific: mrow(mn coefficient, mo times, msup(10, exp)).
    assert_eq!(
        out(&p, "-4200", TargetWriter::MathMl, &[("notation", st("scientific"))]),
        "<mrow><mn>-4</mn><mo>\u{d7}</mo><msup><mn>10</mn><mn>3</mn></msup></mrow>"
    );
}

#[test]
fn omml_semantic_base_structure() {
    assert_eq!(
        with_base("-5", 2, TargetWriter::Omml),
        "<m:sSub><m:sSubPr><m:ctrlPr><w:rPr><w:rFonts w:ascii=\"Cambria Math\" w:hAnsi=\"Cambria Math\"/><w:i/></w:rPr></m:ctrlPr></m:sSubPr><m:e><m:r><m:t>-101</m:t></m:r></m:e><m:sub><m:r><m:t>2</m:t></m:r></m:sub></m:sSub>"
    );
    // The t_tag path is the flat to_s.
    assert_eq!(with_base("-5", 2, TargetWriter::PlainText), "-0b101");
}
