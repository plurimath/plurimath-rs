//! Source Math.parse option validation precedes grammar parsing.
use serde_json::{json, Value};
#[path = "writer/number_formatter/locales.rs"]
mod locale_data;
fn error(class: &str, message: String) -> Value {
    json!({"code":"PARSE_OPTION_ERROR","source_class":class,"message":message})
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
                format!("unsupported locale {locale}"),
            ));
        };
        if *decimal != "." {
            return Err(
                json!({"code":"unported_capability","message":"source comma/non-ASCII decimal locale parsing remains incomplete"}),
            );
        }
    }
    Ok(json!({}))
}
