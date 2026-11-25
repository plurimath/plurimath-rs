//! Opt-in generated XML stages. These models are not the common facade wire.
//!
//! Available on native and wasm32 targets. The facade disables the standalone
//! bindings on its direct Moxml dependency to avoid duplicate WASM exports.
use serde_json::{json, Value};

/// Generated document models, with OMML root namespace declarations retained.
#[derive(Debug, Clone)]
pub enum Document {
    Mathml(Box<mml::v3::Math>),
    Omml(Box<omml::document::OmmlDocument>),
}

/// Parse using the exact stage readers that replay the shared XML corpora.
/// MathML uses the default version 3 model, matching Ruby `Mml.parse`.
pub fn parse(input: &str, format: &str) -> Result<Document, String> {
    match format {
        "mathml" => mml::v3::from_xml(input.as_bytes())
            .map(|model| Document::Mathml(Box::new(model)))
            .map_err(|error| error.to_string()),
        "omml" => omml::document::parse(input.as_bytes())
            .map(|model| Document::Omml(Box::new(model)))
            .map_err(|error| error.to_string()),
        _ => Err(format!("unsupported XML stage format {format}")),
    }
}

impl Document {
    /// Serialize through the stage's Ruby-compatible XML writer.
    pub fn to_xml(&self) -> Result<String, String> {
        match self {
            Self::Mathml(model) => mml::v3::to_xml(model).map_err(|error| error.to_string()),
            Self::Omml(model) => model.to_xml().map_err(|error| error.to_string()),
        }
    }
}

fn failure(code: &str, message: impl ToString) -> Value {
    json!({"abi_version":1,"ok":false,"error":{"code":code,"message":message.to_string()}})
}

/// Bounded JSON transport for the native stage preview.
/// Request: `{"op":"roundtrip","format":"mathml"|"omml","input":"..."}`.
/// Options are not supported by this stage preview and are refused explicitly.
pub fn request_json(bytes: &[u8]) -> Value {
    if bytes.len() > 16 * 1024 * 1024 {
        return failure("invalid_input", "oversized request");
    }
    let value = match crate::json_request::decode(bytes) {
        Ok(value) => value,
        Err(error) => return error,
    };
    if value["op"] != "roundtrip" {
        return failure("invalid_request", "XML stage op must be roundtrip");
    }
    if value
        .get("options")
        .is_some_and(|options| !options.as_object().is_some_and(|map| map.is_empty()))
    {
        return failure("invalid_request", "XML stage options are unsupported");
    }
    let Some(format @ ("mathml" | "omml")) = value["format"].as_str() else {
        return failure("invalid_request", "XML stage format must be mathml or omml");
    };
    let Some(input) = value["input"].as_str() else {
        return failure("invalid_request", "input must be a string");
    };
    let document = match parse(input, format) {
        Ok(document) => document,
        Err(error) => return failure("xml_stage_parse_error", error),
    };
    match document.to_xml() {
        Ok(output) => json!({"abi_version":1,"ok":true,"output":output}),
        Err(error) => failure("xml_stage_write_error", error),
    }
}

/// Native C ABI preview of the generated XML stage round-trip.
/// Free the owned result with `plurimath_facade_free`.
///
/// # Safety
/// A non-null input must point to `len` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn plurimath_facade_xml_stage_json(
    input: *const u8,
    len: usize,
) -> *mut std::ffi::c_char {
    let result = std::panic::catch_unwind(|| {
        if input.is_null() || len > 16 * 1024 * 1024 {
            return failure("invalid_input", "null or oversized request");
        }
        request_json(std::slice::from_raw_parts(input, len))
    })
    .unwrap_or_else(|_| failure("internal_error", "XML stage panic"));
    std::ffi::CString::new(result.to_string())
        .expect("JSON escapes NUL bytes")
        .into_raw()
}
