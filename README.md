# Plurimath numeric Rust kernel

Local numeric JSON kernel with a Rust API, ABI-v1 C library, standalone WASM,
and thin Python/TypeScript transports. This is the qualified numeric subset;
it does not implement the format facade, quantities or GUM and does not accept
the full F40 corpus.

```rust
let wire = plurimath_numeric::evaluate_json(r#"{"schema":"plurimath.evaluate.numeric/1","ast":{"type":"number","text":"2.10"}}"#);
```

Input uses the closed numeric AST: number decimal strings, named variables,
explicit unary/binary grouping, + - * / ^, sqrt/sin/cos/exp/ln/abs, exact string
bindings, strict/approximate mode and optional 1..1000 integral guard digits.
Output is canonical JSON with the reference engine/grammar/schema stamps,
tagged numeric values, boundaries, explicit deferral and typed error codes.

```sh
cargo test --locked
cargo build --locked --lib
cargo build --locked --lib --target wasm32-unknown-unknown
node scripts/check-wasm.mjs
```

The WASM target's precompiled standard library must match rustc. The local
R30 proof used Rust 1.95.0, a checksum-verified target-only component in
`tools/sysroot`, and `RUSTFLAGS="--sysroot $PWD/tools/sysroot"` for the WASM
command. No compiler or global configuration was installed.

C declarations are in `include/plurimath_numeric.h`. The input bytes are
borrowed; each non-null result owns one NUL-terminated UTF-8 JSON buffer and
must be freed exactly once with `plurimath_numeric_result_free`. Null reports
a native runtime failure. Invalid JSON/UTF-8 returns InvalidRequest. All
pointers must address valid memory.

```python
from plurimath_numeric import NumericKernel
kernel = NumericKernel("/absolute/path/libplurimath_numeric.so")
result = kernel.evaluate({"schema": "plurimath.evaluate.numeric/1",
                          "ast": {"type": "number", "text": "2.10"}})
```

Add `bindings/python` to your import path. The wrapper always releases the
native result in `finally` and checks ABI1. It contains no numeric engine.

```typescript
import {numericKernel} from "./bindings/js/kernel.js";
const kernel = await numericKernel(await (await fetch("kernel.wasm")).arrayBuffer());
const wire = kernel.evaluateJson(JSON.stringify(request));
```

The same wrapper runs in Node and browsers, using exported WASM memory and
allocation/free functions. The committed JavaScript/declarations are generated
from `bindings/typescript/kernel.ts`; no JS numeric fallback is present.

`fixtures/numeric-v1.json` preserves the initial 29 expected wires.
`numeric-supplemental-v1.json` contains 105 separately frozen, actual Ruby
oracle probes. These supplemental probes are not an accepted full corpus.
Tests compare every wire byte, including IEEE bits, precision, signed zeros,
boundary order and errors. The reference generation script takes explicit
adapter/input/output paths and does not edit the producer worktree.

Current scope limits: integer exponents are capped at magnitude100000 and
literal decimal exponents at magnitude1000000. These conservative resource
bounds are not the TypeScript engine's separate bit-size preflight policy.
Nonfinite decimal-intermediate recovery remains a known gap; direct final
nonfinite results have typed refusals. Platform proof is Linux native,
Node WASM and Chrome WASM; other native platforms are not verified locally.
