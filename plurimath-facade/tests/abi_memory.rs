use serde_json::{json, Value};
use std::ffi::CStr;

fn allocated_request(bytes: &[u8]) -> Value {
    let input = plurimath_facade::plurimath_facade_alloc(bytes.len());
    assert!(!input.is_null());
    let response = unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), input, bytes.len());
        let result = plurimath_facade::plurimath_facade_json(input, bytes.len());
        assert!(!result.is_null());
        let response = CStr::from_ptr(result).to_bytes().to_vec();
        plurimath_facade::plurimath_facade_free(result);
        plurimath_facade::plurimath_facade_dealloc(input, bytes.len());
        response
    };
    serde_json::from_slice(&response).unwrap()
}

#[test]
fn allocated_utf8_request_roundtrips_and_empty_input_refuses() {
    let model = json!({"class":"Math::Symbols::Symbol","fields":{"value":"α"}});
    let bytes = serde_json::to_vec(&json!({"op":"model","model":model})).unwrap();
    let result = allocated_request(&bytes);
    assert_eq!(result["ok"], true);
    assert_eq!(result["model"], model);
    assert_eq!(allocated_request(&[])["error"]["code"], "invalid_json");
    assert!(plurimath_facade::plurimath_facade_alloc(16 * 1024 * 1024 + 1).is_null());
}
