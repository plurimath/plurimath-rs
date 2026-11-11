# Plurimath native format facade candidate

Native source port progressing toward the full eight-input/six-writer contract. The sealed numeric/quantity/GUM evaluator is a separate library and remains unchanged.

The accepted AsciiMath PARG stage supplies syntax; this package converts its captures into the existing source `{class, fields}` common wire. Arbitrary field values, nulls, lists and plain hashes survive model storage. Common-wire storage is not class-semantic validation.

The initial vertical slice covers numbers, ordinary symbols, arithmetic rows, fractions and round grouping, with six source writers. Static generated symbol data covers1459symbols; context/option behavior still needs qualification. Unported promised formats/classes/options return `unported_capability` and remain incomplete. This refusal is not acceptance of reduced programme scope.

## C ABI

`plurimath_facade_abi_version() ==1`; `plurimath_facade_json(input,length)` returns owned UTF-8 JSON; call `plurimath_facade_free` exactly once. The input remains caller-owned. Maximum request16MiB; capture-transform depth128. Requests:

```json
{"op":"parse","format":"asciimath","input":"x+y","options":{}}
```

```json
{"op":"write","format":"latex","model":{"class":"Math::Number","fields":{"value":"2"}},"options":{}}
```

Success includes `abi_version:1,ok:true` and `model` or `output`. Errors include `abi_version:1,ok:false,error:{code,message}`. Diagnostic/source-offset fidelity remains in progress; do not treat an unported capability as a source syntax refusal.

The `model` operation preserves a source common-wire envelope without rendering. Supported writer format keys are asciimath,latex,unicodemath,html,mathml,omml. Unported input families include the remainder of Ruby's eight input types; no independent grammar or evaluator is substituted.

Focused check: `cargo test --locked --offline --test facade`. Frozen ten-input source oracle provides exact common models and60writer bytes. Full original format acceptance is not claimed.
