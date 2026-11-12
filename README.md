# Plurimath native format facade candidate

Native source port progressing toward the full eight-input/six-writer contract. The sealed numeric/quantity/GUM evaluator is a separate library and remains unchanged.

The accepted AsciiMath PARG stage supplies syntax; this package converts its captures into the existing source `{class, fields}` common wire. Arbitrary field values, nulls, lists and plain hashes survive model storage. Common-wire storage is not class-semantic validation.

The current default-profile AsciiMath conversion matches160 source models. HTML uses the existing source grammar with a portable captured-tag comparison and the source capture transforms:544 source common models and23 source parser refusals match through the native C ABI. The source graph contains no dynamic callback IDs. LaTeX, UnicodeMath, MathML, OMML and UnitsML routes also use accepted parser components. PMA uses the public Amath typed producer and maps measured values and unit terms into the same common model. Its source corpus distinguishes 34 accepted common models and 64 source parse refusals; the typed producer alone accepts four additional terms that the Plurimath source deliberately refuses. Nondefault HTML options and remaining writer/options families stay incomplete. Refusal is not acceptance of a reduced programme scope.

The full eight-input/six-writer contract remains in progress. Common IR is source-backed; generated per-class Rust API parity and universal writer parity are not claimed.

## C ABI

`plurimath_facade_abi_version() ==1`; `plurimath_facade_json(input,length)` returns owned UTF-8 JSON; call `plurimath_facade_free` exactly once. The input remains caller-owned. Maximum request16MiB. Parser resource policies are explicit: AsciiMath capture depth budget1024; HTML preserves its source100-container normalization boundary. Requests:

```json
{"op":"parse","format":"asciimath","input":"x+y","options":{}}
```

PMA numbers use a full stop, and PMA accepts no locale or decimal parse options:

```json
{"op":"parse","format":"pma","input":"unit(m/s)","options":{}}
```

```json
{"op":"write","format":"latex","model":{"class":"Math::Number","fields":{"value":"2"}},"options":{}}
```

Success includes `abi_version:1,ok:true` and `model` or `output`. Errors include `abi_version:1,ok:false,error:{code,message}`. Syntax errors retain declared native offset policies. Diagnostic parity remains profile-specific; do not treat an unported capability as a source syntax refusal.

The `model` operation preserves a source common-wire envelope without rendering. Supported writer format keys are asciimath,latex,unicodemath,html,mathml,omml. Unported input families include the remainder of Ruby's eight input types; no independent grammar or evaluator is substituted.

Focused checks: `cargo test --locked --offline --test facade --test pma --test deep_roundtrip`. Frozen ten-input source oracle provides exact common models and60writer bytes. Full original format acceptance is not claimed.

Source profiles and test evidence are distinct: static symbol maps and guarded templates are not proof of universal writer parity. Templates substitute recursively rendered native children into source-produced text/XML trees, not complete-input answers. The source exporter validated1419profiles through5676source invocations over4145distinct per-profile inputs (finite null domains included), but native acceptance comes from actual frozen/current source differential replays. Named entities, literal-sensitive Text/Mbox/LeftRight and ordinary symbol escapes have dedicated source handlers.

PMA source provenance is in `fixtures/pma/source-provenance.json`. Historical private unit-only PMA artifacts under `src/pma_parse/` are retained as evidence and are not loaded by the active parser. The numeric/quantity/GUM request profile is unchanged and uses its separate evaluator library.
