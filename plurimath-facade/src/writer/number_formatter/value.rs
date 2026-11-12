//! Dynamic Ruby option values and the Ruby coercions the formatter applies
//! to them (`to_s`, `inspect`, `Integer(value.to_s, 10)`, `String#to_i`).

/// One Ruby value as delivered in a formatter option hash.
///
/// The parent adapter decides how request JSON maps onto these variants;
/// `Str` and `Sym` behave identically except for `inspect` and the
/// String-only checks (`localize_number`, `NumberFormatter#supported_locale`
/// accepts both).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RubyValue {
    Nil,
    Bool(bool),
    Int(i64),
    /// A Ruby Float, carried as its exact `Float#to_s` spelling (resolved by
    /// the parent; this lane never re-spells floats).
    Float(String),
    Str(String),
    Sym(String),
    /// Any other Ruby object (Hash, Array, ...): class name and `inspect`.
    Other { class: String, inspect: String },
}

impl RubyValue {
    pub(crate) fn str(s: &str) -> Self {
        RubyValue::Str(s.to_string())
    }
    pub(crate) fn sym(s: &str) -> Self {
        RubyValue::Sym(s.to_string())
    }
    pub(crate) fn is_nil(&self) -> bool {
        matches!(self, RubyValue::Nil)
    }
    /// Ruby truthiness (`nil` and `false` are falsy).
    pub(crate) fn truthy(&self) -> bool {
        !matches!(self, RubyValue::Nil | RubyValue::Bool(false))
    }
    pub(crate) fn is_bool(&self) -> bool {
        matches!(self, RubyValue::Bool(_))
    }
    /// `Object#to_s` (string interpolation).
    pub(crate) fn to_s(&self) -> String {
        match self {
            RubyValue::Nil => String::new(),
            RubyValue::Bool(b) => b.to_string(),
            RubyValue::Int(i) => i.to_string(),
            RubyValue::Float(t) => t.clone(),
            RubyValue::Str(s) | RubyValue::Sym(s) => s.clone(),
            RubyValue::Other { inspect, .. } => inspect.clone(),
        }
    }
    /// `Object#inspect`, used only for structured error payloads.
    pub(crate) fn inspect(&self) -> String {
        match self {
            RubyValue::Nil => "nil".into(),
            RubyValue::Str(s) => inspect_str(s),
            RubyValue::Sym(s) => {
                let plain = !s.is_empty()
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !s.starts_with(|c: char| c.is_ascii_digit());
                if plain {
                    format!(":{s}")
                } else {
                    format!(":{}", inspect_str(s))
                }
            }
            other => other.to_s(),
        }
    }
    /// Ruby `==` against the Integer 10 (`Base::DEFAULT_BASE`).
    pub(crate) fn eq_int(&self, n: i64) -> bool {
        match self {
            RubyValue::Int(i) => *i == n,
            RubyValue::Float(t) => t.parse::<f64>().map(|f| f == n as f64).unwrap_or(false),
            _ => false,
        }
    }
    /// `value.is_a?(String) || value.is_a?(Symbol)`.
    pub(crate) fn string_or_symbol(&self) -> Option<&str> {
        match self {
            RubyValue::Str(s) | RubyValue::Sym(s) => Some(s),
            _ => None,
        }
    }
}

fn inspect_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '#' => out.push('#'),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An ordered Ruby Hash with Symbol keys (keys are the symbol names).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SymbolMap(pub(crate) Vec<(String, RubyValue)>);

impl SymbolMap {
    pub(crate) fn new() -> Self {
        SymbolMap(Vec::new())
    }
    pub(crate) fn from_pairs(pairs: &[(&str, RubyValue)]) -> Self {
        let mut m = SymbolMap::new();
        for (k, v) in pairs {
            m.set(k, v.clone());
        }
        m
    }
    /// `hash[key]` (absent and explicit nil both give `Nil`).
    pub(crate) fn get(&self, key: &str) -> RubyValue {
        self.lookup(key).cloned().unwrap_or(RubyValue::Nil)
    }
    pub(crate) fn lookup(&self, key: &str) -> Option<&RubyValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    /// `hash.key?(key)`.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.lookup(key).is_some()
    }
    /// `hash.fetch(key, default)`.
    pub(crate) fn fetch(&self, key: &str, default: RubyValue) -> RubyValue {
        self.lookup(key).cloned().unwrap_or(default)
    }
    pub(crate) fn set(&mut self, key: &str, value: RubyValue) {
        if let Some(slot) = self.0.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        } else {
            self.0.push((key.to_string(), value));
        }
    }
    /// `self.merge(other)`.
    pub(crate) fn merged(&self, other: &SymbolMap) -> SymbolMap {
        let mut out = self.clone();
        for (k, v) in &other.0 {
            out.set(k, v.clone());
        }
        out
    }
}

/// A Ruby argument that the source expects to be a Hash (or nil).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HashArg {
    Nil,
    Hash(SymbolMap),
    NotHash(RubyValue),
}

impl HashArg {
    pub(crate) fn empty() -> Self {
        HashArg::Hash(SymbolMap::new())
    }
}

/// `Integer(text, 10)` (Kernel#Integer, strict): surrounding whitespace,
/// one sign, optional `0d` prefix, single underscores between digits.
/// Returns `Err(())` where Ruby raises ArgumentError; `Ok(None)` for an
/// integer outside i64 (refused by the caller as a resource limit).
pub(crate) fn kernel_integer_base10(text: &str) -> Result<Option<i64>, ()> {
    if text.contains('\0') {
        return Err(());
    }
    let ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r');
    let t = text.trim_start_matches(ws).trim_end_matches(ws);
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let body = if body.len() > 2 && (body.starts_with("0d") || body.starts_with("0D")) {
        &body[2..]
    } else {
        body
    };
    let bytes = body.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() || !bytes[bytes.len() - 1].is_ascii_digit() {
        return Err(());
    }
    let mut digits = String::new();
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'0'..=b'9' => digits.push(*b as char),
            b'_' if bytes[i - 1] != b'_' => {}
            _ => return Err(()),
        }
    }
    let magnitude: Option<i128> = digits.trim_start_matches('0').parse::<i128>().ok().or(
        if digits.trim_start_matches('0').is_empty() { Some(0) } else { None },
    );
    Ok(magnitude
        .map(|m| if neg { -m } else { m })
        .and_then(|v| i64::try_from(v).ok()))
}

/// Ruby `String#to_i` (lenient, base 10) — used for `exponent_text.to_i`.
pub(crate) fn string_to_i_lenient(text: &str) -> Option<i64> {
    let (neg, digits, _) = string_to_i_digits(text, 10);
    let dec = super::digits::base_values_to_decimal(&digits, 10);
    let trimmed = dec.trim_start_matches('0');
    if trimmed.len() > 18 {
        return None;
    }
    let v: i64 = if trimmed.is_empty() { 0 } else { trimmed.parse().ok()? };
    Some(if neg { -v } else { v })
}

/// Ruby `String#to_i(base)` prefix scan: leading whitespace, one sign, an
/// optional radix prefix matching `base`, then digits with single
/// underscores between them; stops at the first invalid character.
/// `base == 0` auto-detects the radix from the prefix (`0x`, `0b`, `0o`,
/// `0d`, leading `0` = octal, else decimal), as Ruby does.
/// Returns `(negative, digit values, effective radix)`.
pub(crate) fn string_to_i_digits(text: &str, base: u32) -> (bool, Vec<u32>, u32) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() && matches!(chars[i], ' ' | '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r') {
        i += 1;
    }
    let mut neg = false;
    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
        neg = chars[i] == '-';
        i += 1;
    }
    let mut base = base;
    if base == 0 {
        base = match (chars.get(i), chars.get(i + 1).map(|c| c.to_ascii_lowercase())) {
            (Some('0'), Some('x')) => 16,
            (Some('0'), Some('b')) => 2,
            (Some('0'), Some('o')) => 8,
            (Some('0'), Some('d')) => 10,
            (Some('0'), Some(_)) => {
                i += 1;
                8
            }
            _ => 10,
        };
    }
    if i + 1 < chars.len() && chars[i] == '0' {
        let p = chars[i + 1].to_ascii_lowercase();
        let matches_prefix = matches!((base, p), (16, 'x') | (2, 'b') | (8, 'o') | (10, 'd'));
        if matches_prefix {
            i += 2;
        }
    }
    let value = |c: char| c.to_digit(36).filter(|v| *v < base);
    let mut out = Vec::new();
    while i < chars.len() {
        if let Some(v) = value(chars[i]) {
            out.push(v);
            i += 1;
        } else if chars[i] == '_'
            && !out.is_empty()
            && i + 1 < chars.len()
            && value(chars[i + 1]).is_some()
        {
            i += 1;
        } else {
            break;
        }
    }
    (neg, out, base)
}
