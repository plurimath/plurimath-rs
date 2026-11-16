//! Source UnicodeMath char/diacritics and unicoded-font captures (Ruby 0a1df68).
//! These are symbol constructors and ordered lists, not font-style wrappers.
use super::{incomplete, keys, simple, symbol, Rec};
use crate::model::number;
use serde_json::{Map, Value};

/// Return `None` for capture shapes without a matching source rule.
pub(super) fn tx(m: &Map<String, Value>, rec: Rec) -> Result<Option<Vec<Value>>, Value> {
    if keys(m, &["unicoded_font_class", "symbol"]) {
        let selector = &m["unicoded_font_class"];
        let font = if let Some(text) = selector.as_str() {
            text
        } else if selector.as_object().is_some_and(|m| {
            keys(m, &["mitBbb"]) && !m["mitBbb"].is_object() && !m["mitBbb"].is_array()
        }) {
            // Source `mitBbb: simple` builds :mitBbb, including the nil marker
            // produced by umath for the literal Unicode letter forms.
            "mitBbb"
        } else {
            return Err(incomplete("unported UnicodeMath unicoded font selector"));
        };
        let text = m["symbol"]
            .as_str()
            .ok_or_else(|| incomplete("non-text unicoded font symbol"))?;
        // Complete Constants::UNICODED_FONTS table, including source fallback.
        let mapped = match (font, text) {
            ("script", "H") => "&#x210b;",
            ("fraktur", "H") => "&#x210c;",
            ("double", "H") => "&#x210d;",
            ("mitBbb", "D") => "&#x2145;",
            ("mitBbb", "d") => "&#x2146;",
            ("mitBbb", "e") => "&#x2147;",
            ("mitBbb", "i") => "&#x2148;",
            ("mitBbb", "j") => "&#x2149;",
            _ => text,
        };
        return Ok(Some(vec![symbol(mapped)]));
    }

    // Source rules.rb 406–423 and 2101–2116. All capture sets are exact;
    // order comes from the source rule, never from JSON map iteration.
    let has_diacritics = keys(m, &["char", "diacritics"])
        || keys(m, &["char", "diacritics", "number"])
        || keys(m, &["char", "diacritics", "alphanumeric"]);
    if !has_diacritics && !keys(m, &["char", "number"]) && !keys(m, &["char", "alphanumeric"]) {
        return Ok(None);
    }
    let first = simple(rec(&m["char"])?, "char")?;
    let mut values = vec![first];
    if has_diacritics {
        let diacritics = rec(&m["diacritics"])?;
        // The alphanumeric continuation has only a `simple` source rule;
        // unlike the number continuation, a sequence must remain unbuilt.
        if m.contains_key("alphanumeric") && (m["diacritics"].is_array() || diacritics.len() != 1) {
            return Ok(None);
        }
        values.extend(diacritics);
    }
    if let Some(n) = m.get("number") {
        let text = n
            .as_str()
            .ok_or_else(|| incomplete("non-text char number continuation"))?;
        values.push(number(text));
    }
    if let Some(alpha) = m.get("alphanumeric") {
        let text = alpha
            .as_str()
            .ok_or_else(|| incomplete("non-text char alphanumeric continuation"))?;
        values.push(symbol(text));
    }
    Ok(Some(values))
}
