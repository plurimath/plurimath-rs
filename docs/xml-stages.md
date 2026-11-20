# Generated XML stage preview

With the `xml-stages` feature on native and wasm32 targets, the facade exposes the
upstream generated stage readers and Ruby-compatible XML writers:

```rust
use plurimath_facade::xml_parse::stage;
let document = stage::parse("<math><mi>x</mi></math>", "mathml")?;
let xml = document.to_xml()?;
# Ok::<(), String>(())
```

`Document::Mathml` holds the MathML 3 model used by Ruby `Mml.parse`;
`Document::Omml` holds the generated oMath/oMathPara document, including its
root namespace declarations. Callers can inspect or edit these typed models
before calling `to_xml`. The stage reader and writer handle XML layout,
comments and namespace retention. The facade does not duplicate those rules.

The native C entrypoint `plurimath_facade_xml_stage_json(input, length)` accepts
UTF-8 JSON `{"op":"roundtrip","format":"mathml","input":"<math/>"}` (or
`omml`). It returns `{abi_version:1,ok:true,output:...}` or an error envelope.
The input is borrowed; free the returned string exactly once with
`plurimath_facade_free`. The existing 16 MiB request cap and bounded JSON
transport apply. Only absent or empty-object options are accepted.

These are stage models, distinct from the facade's common `{class,fields}`
wire. Existing common-model `parse`/`write` requests retain their routes.
The stage preview does not establish common-model or full format parity.

## Temporary dependencies and merge hold

This change stays **draft** until lutaml/lutaml-rs#9 and #10 merge, the XML
stage PRs land, and all temporary pins move to verified main commits:

- mml-rs#4: `890b13ae4d486d67d5d56a8502bbb4502540aed0`.
- omml-rs#7 (includes #5): `fb475290b6657d4a53fbf9dbf86f19902c744035`.
- Both depend on the lutaml-rs#10 fork head
  `02340c77d8034826f285927520eda83cb89d4955`.

The facade pins Moxml `4e9c1b2869f95b3c5174d52a84ab8c172e78ceb5` with
`default-features = false`, using the optional standalone WASM bindings change
in lutaml/moxml-rs#28. The stage SDK still pins Moxml `c55a7ec`; only that
revision exports its wasm-bindgen face. Both revisions retain their Rust
model/document APIs, including the facade's source parser policies.
This removes the duplicate-export linker blocker without a same-source Cargo
patch. The Moxml pin is temporary until that PR merges; keep this facade PR
draft until all upstream holds above and this new pin are resolved.

On wasm32, the typed Rust stage API and raw C ABI are available. This does not
add a TypeScript wrapper or route stage models through the common facade wire.

## Focused verification

`cargo test --locked -p plurimath-facade --features xml-stages --test xml_stage_smoke`
checks 11 shared-stage rows through the typed API and JSON entrypoint. The
sample covers Unicode entities, unnamespaced MathML, mixed-content layout,
comments, OMML namespace order and run content, both OMML roots, and refusals.
The C ABI test checks result ownership and invalid buffers.

`python3 scripts/export_xml_stage_smoke.py --check` verifies the sample against
five checksummed payloads from the same generations the upstream stages replay
(MathML generation 3, OMML generation 2). It needs PyYAML and network access.
It copies recorded outputs unchanged and does not replay a full corpus.
Existing CI compiles all features and runs the tests; the existing WASM job
checks default and all-feature builds. Nothing here publishes a package.
