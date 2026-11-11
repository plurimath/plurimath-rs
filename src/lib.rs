pub mod model;
pub mod parser;
pub mod symbols;
pub mod writer;
use serde_json::{json, Value};
pub fn stage_tree(input: &str) -> Result<Value, String> {
    amath::Parser::new()
        .map_err(|e| e.to_string())?
        .parse_legacy(input)
        .map(|p| p.canonical())
        .map_err(|e| e.to_string())
}
pub fn request(v: &Value) -> Value {
    let result: Result<Value, String> = (|| {
        let op = v["op"].as_str().ok_or("missing op")?;
        match op {
            "parse" => Ok(
                json!({"model":parser::parse(v["input"].as_str().ok_or("missing input")?,v["format"].as_str().ok_or("missing format")?,v.get("options").unwrap_or(&json!({})))?}),
            ),
            "write" => Ok(
                json!({"output":writer::write(&v["model"],v["format"].as_str().ok_or("missing format")?,v.get("options").unwrap_or(&json!({})))?}),
            ),
            "model" => Ok(json!({"model":model::Model::from_value(&v["model"])?.value()})),
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
            json!({"abi_version":1,"ok":false,"error":{"code":"unported_capability","message":message}})
        }
    }
}
#[no_mangle]
pub extern "C" fn plurimath_facade_abi_version() -> u32 {
    1
}
#[no_mangle]
pub unsafe extern "C" fn plurimath_facade_json(
    input: *const u8,
    len: usize,
) -> *mut std::ffi::c_char {
    let result=std::panic::catch_unwind(||{if input.is_null()||len>16*1024*1024{return json!({"abi_version":1,"ok":false,"error":{"code":"invalid_input","message":"null or oversized request"}})}let bytes=std::slice::from_raw_parts(input,len);match serde_json::from_slice::<Value>(bytes){Ok(v)=>request(&v),Err(e)=>json!({"abi_version":1,"ok":false,"error":{"code":"invalid_json","message":e.to_string()}})}}).unwrap_or_else(|_|json!({"abi_version":1,"ok":false,"error":{"code":"internal_error","message":"native panic"}}));
    std::ffi::CString::new(result.to_string())
        .unwrap()
        .into_raw()
}
#[no_mangle]
pub unsafe extern "C" fn plurimath_facade_free(p: *mut std::ffi::c_char) {
    if !p.is_null() {
        drop(std::ffi::CString::from_raw(p))
    }
}
