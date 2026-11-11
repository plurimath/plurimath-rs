# Plurimath native format facade candidate

Native source port progressing toward the full eight-input/six-writer contract. The sealed numeric/quantity/GUM evaluator is a separate library and remains unchanged.

The accepted AsciiMath PARG stage supplies syntax; this package converts its captures into the existing source `{class, fields}` common wire. Arbitrary field values, nulls, lists and plain hashes survive model storage. Common-wire storage is not class-semantic validation.

The current default-profile AsciiMath conversion matches160 source models. HTML uses the existing source grammar with a portable captured-tag comparison and the source capture transforms:544 source common models and23 source parser refusals match through the native C ABI. The source graph contains no dynamic callback IDs. LaTeX, UnicodeMath, MathML, OMML, UnitsML and unit-only PMA routes also use accepted parser components; their remaining source families and writer/options profiles stay incomplete. Measured PMA, nondefault HTML options and remaining writer classes/options return explicit incomplete errors. Refusal is not acceptance of a reduced programme scope.

The full eight-input/six-writer contract remains in progress. Common IR is source-backed; generated per-class Rust API parity and universal writer parity are not claimed.

## C ABI

`plurimath_facade_abi_version() ==1`; `plurimath_facade_json(input,length)` returns owned UTF-8 JSON; call `plurimath_facade_free` exactly once. The input remains caller-owned. Maximum request16MiB. Parser resource policies are explicit: AsciiMath capture depth budget1024; HTML preserves its source100-container normalization boundary. Requests:

```json
{"op":"parse","format":"asciimath","input":"x+y","options":{}}
```

```json
{"op":"write","format":"latex","model":{"class":"Math::Number","fields":{"value":"2"}},"options":{}}
```

Success includes `abi_version:1,ok:true` and `model` or `output`. Errors include `abi_version:1,ok:false,error:{code,message}`. Syntax errors retain declared native offset policies. Diagnostic parity remains profile-specific; do not treat an unported capability as a source syntax refusal.

The `model` operation preserves a source common-wire envelope without rendering. Supported writer format keys are asciimath,latex,unicodemath,html,mathml,omml. Unported input families include the remainder of Ruby's eight input types; no independent grammar or evaluator is substituted.

Focused check: `cargo test --locked --offline --test facade`. Frozen ten-input source oracle provides exact common models and60writer bytes. Full original format acceptance is not claimed.

Source profiles and test evidence are distinct: static symbol maps and guarded templates are not proof of universal writer parity. Templates substitute recursively rendered native children into source-produced text/XML trees, not complete-input answers. The source exporter validated1419profiles through5676source invocations over4145distinct per-profile inputs (finite null domains included), but native acceptance comes from actual frozen/current source differential replays. Named entities, literal-sensitive Text/Mbox/LeftRight and ordinary symbol escapes have dedicated source handlers.
