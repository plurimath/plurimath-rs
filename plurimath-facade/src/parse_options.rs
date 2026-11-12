//! Source Math.parse option validation precedes grammar parsing.
use serde_json::{json, Value};
use crate::locale_data;
fn error(class: &str, message: String) -> Value {
    json!({"code":"PARSE_OPTION_ERROR","source_class":class,"message":message})
}
fn unsupported_locale(locale: &Value) -> Result<String, Value> {
    use crate::source_values::{decode, inspect, Domain, Profile, RubyValue};
    let inspect_error = |message: String| json!({"code":"source_value_error","message":message});
    let locale =
        inspect(&decode(locale, Domain::Literal, Profile::Canonical).map_err(inspect_error)?)
            .map_err(inspect_error)?;
    let supported = locale_data::LOCALES
        .iter()
        .map(|entry| inspect(&RubyValue::Symbol(entry.0.into())))
        .collect::<Result<Vec<_>, _>>()
        .map_err(inspect_error)?
        .join(", ");
    Ok(format!(
        "[plurimath] Unsupported locale {locale}. Supported locales are: {supported}."
    ))
}

pub(crate) fn normalize(format: &str, value: Option<&Value>) -> Result<Value, Value> {
    let Some(value) = value else {
        return Ok(json!({}));
    };
    let options = value.as_object().ok_or_else(|| {
        error(
            "Plurimath::Math::ParseOptionError",
            "parse options must be an object".into(),
        )
    })?;
    let unknown: Vec<_> = options.keys().filter(|k| k.as_str() != "locale").collect();
    if !unknown.is_empty() {
        return Err(error(
            "Plurimath::Math::ParseOptionError",
            format!(
                "unknown parse option{}: {}; supported parse options are :locale",
                if unknown.len() == 1 { "" } else { "s" },
                unknown
                    .iter()
                    .map(|k| format!(":{k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if let Some(locale) = options.get("locale") {
        if !["asciimath", "html", "latex", "unicode"].contains(&format) {
            return Err(error("Plurimath::Math::ParseOptionError",format!("parse option :locale is not supported for :{format}; supported input types are :asciimath, :html, :latex, :unicode")));
        }
        if locale.is_null() {
            return Ok(json!({}));
        }
        let Some((_, decimal, _)) = locale
            .as_str()
            .and_then(|locale| locale_data::LOCALES.iter().find(|entry| entry.0 == locale))
        else {
            return Err(error(
                "Plurimath::Errors::UnsupportedLocale",
                unsupported_locale(locale)?,
            ));
        };
        if *decimal != "." {
            // Preserve the validated source locale for the stage grammar. The
            // decimal is selected per request; shared parser state is unchanged.
            return Ok(json!({"locale": locale}));
        }
    }
    Ok(json!({}))
}

/// Decimal separator for source-validated parser options.
///
/// Call `normalize` before this accessor when accepting caller options.
pub(crate) fn decimal(options: &Value) -> &'static str {
    options
        .get("locale")
        .and_then(Value::as_str)
        .and_then(|locale| locale_data::LOCALES.iter().find(|entry| entry.0 == locale))
        .map_or(".", |entry| entry.1)
}

#[cfg(test)]
mod locale_tests {
    use super::*;

    #[test]
    fn source_validation_precedes_decimal_selection() {
        for value in [json!({}), json!({"locale":null}), json!({"locale":"en"})] {
            assert_eq!(normalize("html", Some(&value)).unwrap(), json!({}));
        }
        assert_eq!(decimal(&normalize("html", None).unwrap()), ".");
        assert_eq!(
            normalize("html", Some(&json!({"locale":"missing"}))).unwrap_err()["source_class"],
            "Plurimath::Errors::UnsupportedLocale"
        );
        assert_eq!(
            normalize("mathml", Some(&json!({"locale":"de"}))).unwrap_err()["source_class"],
            "Plurimath::Math::ParseOptionError"
        );
        assert_eq!(
            normalize("html", Some(&json!({"extra":true}))).unwrap_err()["source_class"],
            "Plurimath::Math::ParseOptionError"
        );
    }
}
