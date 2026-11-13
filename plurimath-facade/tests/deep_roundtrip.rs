use serde_json::{json, Value};
fn call(value: &Value) -> Value {
    let wire = serde_json::to_vec(value).unwrap();
    unsafe {
        let pointer = plurimath_facade::plurimath_facade_json(wire.as_ptr(), wire.len());
        assert!(!pointer.is_null());
        let output = std::ffi::CStr::from_ptr(pointer).to_bytes().to_vec();
        plurimath_facade::plurimath_facade_free(pointer);
        let mut decoder = serde_json::Deserializer::from_slice(&output);
        decoder.disable_recursion_limit();
        <Value as serde::Deserialize>::deserialize(&mut decoder).unwrap()
    }
}
#[test]
fn parsed_deep_groups_roundtrip_through_public_json_on_default_test_thread() {
    for depth in [16, 64, 128] {
        let input = format!("{}x{}", "(".repeat(depth), ")".repeat(depth));
        let parsed = call(&json!({"op":"parse","format":"asciimath","input":input}));
        assert_eq!(parsed["ok"], true, "depth{depth}: {parsed}");
        for format in [
            "asciimath",
            "latex",
            "html",
            "unicodemath",
            "mathml",
            "omml",
        ] {
            let expected =
                plurimath_facade::writer::write(&parsed["model"], format, &json!({})).unwrap();
            let output = call(&json!({"op":"write","format":format,"model":parsed["model"]}));
            assert_eq!(output["ok"], true, "depth{depth} {format}: {output}");
            assert_eq!(output["output"], expected);
        }
    }
    let resource = call(
        &json!({"op":"parse","format":"asciimath","input":format!("{}x{}","(".repeat(256),")".repeat(256))}),
    );
    assert_eq!(resource["ok"], false);
    assert_eq!(resource["error"]["code"], "resource_limit");
    assert_eq!(
        call(&json!({"op":"parse","format":"asciimath","input":"1"}))["ok"],
        true
    );
}

#[test]
fn maximum_transport_nesting_runs_on_default_test_thread() {
    let wire = format!("{}0{}", "[".repeat(1024), "]".repeat(1024));
    unsafe {
        let pointer = plurimath_facade::plurimath_facade_json(wire.as_ptr(), wire.len());
        assert!(!pointer.is_null());
        let output = std::ffi::CStr::from_ptr(pointer).to_bytes().to_vec();
        plurimath_facade::plurimath_facade_free(pointer);
        let response: Value = serde_json::from_slice(&output).unwrap();
        assert_ne!(response["error"]["code"], "invalid_json");
        assert_ne!(response["error"]["code"], "resource_limit");
    }
}

#[test]
fn transport_node_budget_and_reserved_literal_keys_are_explicit() {
    let wire = format!("[{}0]", "0,".repeat(1_000_000));
    unsafe {
        let pointer = plurimath_facade::plurimath_facade_json(wire.as_ptr(), wire.len());
        assert!(!pointer.is_null());
        let output = std::ffi::CStr::from_ptr(pointer).to_bytes().to_vec();
        plurimath_facade::plurimath_facade_free(pointer);
        let response: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(response["error"]["code"], "resource_limit");
        assert_eq!(response["error"]["resource"]["kind"], "json_nodes");
        assert_eq!(response["error"]["resource"]["max_nodes"], 1_000_000);
    }
    let literal = json!({"op":"model","model":{"class":"Math::Number","fields":{"value":{"$serde_json::private::RawValue":"not JSON"}}}});
    // The raw_value Cargo feature reserves a marker in Serde Value's own
    // response deserializer. Compare actual C reply bytes and the public Rust
    // value API; neither may infer a literal Hash as raw JSON.
    let input = serde_json::to_vec(&literal).unwrap();
    assert_eq!(
        plurimath_facade::request_json(&input)["model"],
        literal["model"]
    );
    unsafe {
        let pointer = plurimath_facade::plurimath_facade_json(input.as_ptr(), input.len());
        assert!(!pointer.is_null());
        let output = std::ffi::CStr::from_ptr(pointer).to_bytes().to_vec();
        plurimath_facade::plurimath_facade_free(pointer);
        let expected =
            serde_json::to_vec(&json!({"model":literal["model"],"abi_version":1,"ok":true}))
                .unwrap();
        assert_eq!(output, expected);
    }
    assert_eq!(
        call(&json!({"op":"parse","format":"asciimath","input":"1"}))["ok"],
        true
    );
}

#[test]
fn split_clone_validation_preserves_default_stack_deep_groups() {
    let input = format!("{}x{}", "(".repeat(128), ")".repeat(128));
    let parsed = call(&json!({"op":"parse","format":"asciimath","input":input}));
    assert_eq!(parsed["ok"], true);
    for format in ["mathml", "omml"] {
        let output = call(
            &json!({"op":"write","format":format,"model":parsed["model"],"options":{"splitOnLinebreak":true}}),
        );
        assert_eq!(output["ok"], true, "{format}: {output}");
    }
    assert_eq!(
        call(&json!({"op":"parse","format":"asciimath","input":"1"}))["ok"],
        true
    );
}
