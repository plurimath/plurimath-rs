pub mod html_parse;
mod json_request;
#[path = "writer/number_formatter/locales.rs"]
mod locale_data;
pub mod model;
mod parse_options;
pub mod parser;
pub mod pma_parse;
pub mod source_values;
pub mod symbols;
pub mod text_parse;
pub mod units_parse;
pub mod writer;
pub mod xml_parse;
use serde_json::{json, Value};
pub fn stage_tree(input: &str) -> Result<Value, String> {
    amath::Parser::new()
        .map_err(|e| e.to_string())?
        .parse_legacy(input)
        .map(|p| p.canonical())
        .map_err(|e| e.to_string())
}
pub fn request(v: &Value) -> Value {
    let mut response = request_inner(v);
    // Math.parse wraps grammar syntax failures in its public ParseError.
    // Preserve typed native resource errors and their distinct envelopes.
    if v["op"] == "parse"
        && response["ok"] == false
        && response["error"]["source_class"].is_null()
        && matches!(
            response["error"]["code"].as_str(),
            Some("syntax_error" | "PARSE_ERROR")
        )
    {
        response["error"]["source_class"] = json!("Plurimath::Math::ParseError");
    }
    response
}
fn request_inner(v: &Value) -> Value {
    let parse_format = match v["format"].as_str().unwrap_or("") {
        "unicodemath" => "unicode",
        format => format,
    };
    let normalized_parse_options = if v["op"] == "parse"
        && [
            "asciimath",
            "html",
            "latex",
            "unicode",
            "mathml",
            "omml",
            "unitsml",
            "pma",
        ]
        .contains(&parse_format)
    {
        match parse_options::normalize(parse_format, v.get("options")) {
            Ok(options) => options,
            Err(error) => return json!({"abi_version":1,"ok":false,"error":error}),
        }
    } else {
        json!({})
    };
    if v["op"] == "parse" && parse_format == "html" {
        let Some(input) = v["input"].as_str() else {
            return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
        };
        return match html_parse::parse(input, &normalized_parse_options) {
            Ok(model) => {
                let mut response = json!({"abi_version":1,"ok":true});
                response["model"] = model;
                response
            }
            Err(error) => json!({"abi_version":1,"ok":false,"error":error}),
        };
    }

    if v["op"] == "parse" && parse_format == "pma" {
        let Some(input) = v["input"].as_str() else {
            return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
        };
        return match pma_parse::parse(input, &normalized_parse_options) {
            Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
            Err(error) => json!({"abi_version":1,"ok":false,"error":error}),
        };
    }

    if v["op"] == "parse" && parse_format == "unitsml" {
        let Some(input) = v["input"].as_str() else {
            return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
        };
        return match units_parse::parse(input, &normalized_parse_options) {
            Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
            Err(error) => json!({"abi_version":1,"ok":false,"error":error}),
        };
    }

    if v["op"] == "parse" && ["latex", "unicode"].contains(&parse_format) {
        let Some(input) = v["input"].as_str() else {
            return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
        };
        return match text_parse::parse(input, parse_format, &normalized_parse_options) {
            Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
            Err(error) => json!({"abi_version":1,"ok":false,"error":error}),
        };
    }

    if v["op"] == "parse" && ["mathml", "omml"].contains(&parse_format) {
        let Some(input) = v["input"].as_str() else {
            return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_request","message":"input must be a string"}});
        };
        return match xml_parse::parse(input, parse_format, &normalized_parse_options) {
            Ok(model) => json!({"abi_version":1,"ok":true,"model":model}),
            Err(error) => json!({"abi_version":1,"ok":false,"error":error}),
        };
    }

    if v["op"] == "parse" {
        return parser::response(
            &json!({"input":v["input"],"format":if v["format"].is_string(){json!(parse_format)}else{v["format"].clone()},"options":normalized_parse_options}),
        );
    }
    let result: Result<Value, String> = (|| {
        let op = v["op"].as_str().ok_or("missing op")?;
        match op {
            "parse" => Ok(
                json!({"model":parser::parse(v["input"].as_str().ok_or("missing input")?,v["format"].as_str().ok_or("missing format")?,v.get("options").unwrap_or(&json!({})))?}),
            ),
            "write" => Ok(
                json!({"output":writer::write(&source_values::request_model(v)?,v["format"].as_str().ok_or("missing format")?,v.get("options").unwrap_or(&json!({})))?}),
            ),
            "model" => Ok(json!({"model":model::Model::from_value(&v["model"])?.value()})),
            "source_value" => source_values::value_request(v),
            "source_model" => {
                source_values::document_model(&v["model"])?;
                Ok(json!({"model":v["model"]}))
            }
            _ => Err(format!("unported promised facade operation {op}")),
        }
    })();
    match result {
        Ok(mut x) => {
            x["abi_version"] = json!(1);
            x["ok"] = json!(true);
            x
        }
        Err(message) => {
            if let Some(value) = message
                .strip_prefix("formatter_error_json:")
                .or_else(|| message.strip_prefix("formatter_constructor_error_json:"))
            {
                let error: Value=serde_json::from_str(value).unwrap_or_else(|_|json!({"code":"invalid_request","message":"invalid formatter error envelope"}));
                json!({"abi_version":1,"ok":false,"error":error})
            } else if let Some(message) = message.strip_prefix("source_argument_error: ") {
                json!({"abi_version":1,"ok":false,"error":{"code":"render_error","message":message,"source_class":"ArgumentError"}})
            } else if let Some(message) = message.strip_prefix("source_no_method_error: ") {
                json!({"abi_version":1,"ok":false,"error":{"code":"render_error","message":message,"source_class":"NoMethodError"}})
            } else if let Some(message) = message.strip_prefix("source_render_error: ") {
                json!({"abi_version":1,"ok":false,"error":{"code":"render_error","message":message,"source_class":if source_values::request_root_class(v)==Some("Math::Formula"){"Plurimath::Math::ParseError"}else{"NoMethodError"}}})
            } else {
                json!({"abi_version":1,"ok":false,"error":{"code":"unported_capability","message":message}})
            }
        }
    }
}
#[no_mangle]
pub extern "C" fn plurimath_facade_abi_version() -> u32 {
    1
}
/// Decode one bounded UTF-8 JSON request through the same transport as the C ABI.
pub fn request_json(bytes: &[u8]) -> Value {
    if bytes.len() > 16 * 1024 * 1024 {
        return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_input","message":"oversized request"}});
    }
    match json_request::decode(bytes) {
        Ok(value) => request(&value),
        Err(error) => error,
    }
}

/// # Safety
/// A non-null input must point to `len` readable bytes for this call.
/// Release the returned owned pointer once using `plurimath_facade_free`.
#[no_mangle]
pub unsafe extern "C" fn plurimath_facade_json(
    input: *const u8,
    len: usize,
) -> *mut std::ffi::c_char {
    let result=std::panic::catch_unwind(||{if input.is_null()||len>16*1024*1024{return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_input","message":"null or oversized request"}})}let bytes=std::slice::from_raw_parts(input,len);request_json(bytes)}).unwrap_or_else(|_|json!({"abi_version":1,"ok":false,"error":{"code":"internal_error","message":"native panic"}}));
    std::ffi::CString::new(result.to_string())
        .unwrap()
        .into_raw()
}
/// # Safety
/// A non-null pointer must be an unreleased return from `plurimath_facade_json`.
#[no_mangle]
pub unsafe extern "C" fn plurimath_facade_free(p: *mut std::ffi::c_char) {
    if !p.is_null() {
        drop(std::ffi::CString::from_raw(p))
    }
}
