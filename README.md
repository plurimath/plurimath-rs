# Plurimath numeric Rust kernel

Local numeric JSON kernel with a Rust API, ABI-v1 C library, standalone WASM,
and thin Python/TypeScript transports. The optional quantity profile
uses the same ordinary AST. The format facade, GUM and full F40 qualification
are outside this package's scope.

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

Current scope limits: nontrivial integer powers are capped at exponent magnitude 100,000 and
literal decimal exponents at magnitude 1,000,000. These conservative resource
bounds are not the TypeScript engine's separate bit-size preflight policy.
Nonfinite decimal intermediates remain available for subsequent recovery; only
nonfinite final results receive the reference typed refusal. The 13 nonfinite-recovery, 9 large-integer identity/logarithm and 3 validation-
priority probes are separate source-derived supplemental sets.
Platform proof is Linux native,
Node WASM and Chrome WASM; other native platforms are not verified locally.

## Quantity requests

Build the local Rust/C package with `cargo build --locked --features quantity`.
The feature uses the adjacent AsciiUnits and UnitsML Rust packages. The default build supports the frozen numeric profile. A quantity request
to a default build returns `UnavailableProfileError`.

```json
{
  "schema": "plurimath.evaluate.quantity/1",
  "ast": {
    "type": "binary",
    "operator": "*",
    "left": {"type": "variable", "name": "v"},
    "right": {"type": "variable", "name": "i"}
  },
  "bindings": {
    "v": {"value": "2", "unit": "kV"},
    "i": {"value": "3", "unit": "A"}
  },
  "convert_to": "kW"
}
```

Pass this request to the same `evaluate_json` Rust function or
`plurimath_numeric_evaluate` C function. The result contains the expression
quantity, a converted quantity when requested, and exact accuracy. Each
quantity reports a rational magnitude string, canonical magnitude, exact
dimension exponents, registered dimension ID when available, scale, and
resolved unit and prefix terms. This example returns a magnitude of 6 in kW,
with canonical magnitude 6000 and dimension ID NISTd16.

Scalar bindings use the numeric profile's strict decimal strings. A unit
binding supplies a decimal `value`, an AsciiUnits `unit`, and optional boolean
`difference`. Evaluation supports numbers, variables, unary signs, explicit
grouping, + − × ÷, and dimensionless integer exponents of magnitude at most
4096. Power also checks a 1,000,000-bit bound on its unreduced magnitude and
factor. Every intermediate uses the UnitsML quantity type, including bare
numbers. Approximate and guarded evaluation, calls, and custom conversion
registers are outside this profile. AsciiUnits syntax refusals preserve the
parser location. Inconsistent register dimensions, missing conversion paths
and illegal scale operations produce typed refusals. A zero divisor returns
`DivisionByZeroError` with class `indeterminate`.

`fixtures/quantity-f40.json` projects the frozen Ruby F40 voltage-current
conversion case onto the ordinary AST. The result verifies conversion to kW
and the power dimension vector. Named quantity-kind classification is outside
this profile.

### Quantity WASM

Build with `cargo build --locked --lib --features quantity --target wasm32-unknown-unknown`.
Use the `wasm-bindgen` CLI version pinned by Cargo.lock with `--target web`
to generate the host glue and processed WASM. The AsciiUnits runtime uses the
platform random source through those generated imports. Use the existing
ABI transport with the initialized exports.

```typescript
import init from "./wasm/plurimath_numeric.js";
import {numericKernelFromExports} from "./bindings/js/kernel.js";

const kernel = numericKernelFromExports(await init());
const wire = kernel.evaluateJson(JSON.stringify(request));
```

The default numeric-only WASM retains the existing `numericKernel(bytes)`
loader.

## Exact statistics and uncertainty

The same Rust and C ABI entry points accept the `plurimath.evaluate.gum/1`
profile. A request names a method and its parameters.

```json
{"schema":"plurimath.evaluate.gum/1","method":"type_a","parameters":{"observations":["1","2","3"]}}
```

Exact inputs accept JSON integers, decimal or fraction strings, and tagged
`{"decimal":"0.1"}` or `{"rational":["1","10"]}` values. Results use tagged
rationals for exact values and decimal text where a method declares rounding.
Errors carry a typed code and message.

- `type_a`, `type_a_covariance`, `series`, and `grouped_type_a` compute
  sample and mean statistics. Series select axis 0 or 1 and accept a
  zero-based `[start,length]` window. Grouped estimates require an explicit
  variance model.
- `component` derives variance from a declared PDF. Student-t PDF degrees of
  freedom remain separate from variance-estimate degrees of freedom.
- `propagate` accepts an estimate, component variances, and gradient.
  `propagate_covariance` also validates a symmetric, positive-semidefinite
  covariance matrix. `propagate_vector` retains output covariance. First-order
  independent propagation reports Welch–Satterthwaite status.
- `analyze` accepts the ordinary AST and an ordered `inputs` array of
  `{name,estimate,component}` records. It computes exact first through third
  derivatives for rational arithmetic and constant integer powers. Calls are
  refused. Second-order independent propagation requires normal components.
  Second-order correlated propagation requires explicit multivariate normality
  and complete symmetric derivative tensors.
- `coverage` uses a portable Rust binary64 backend, with normal or
  Student-t factors, probability 0.5–0.999, Student-t degrees of freedom
  1–10000, and 1–8 significant digits.
- `monte_carlo` accepts an ordinary AST, ordered `{name,pdf}` inputs, four
  explicit seeds, independent sampling, probability, interval selection,
  significant and guard digits, and a sample budget. It uses centered exact
  polynomials with binary64 samples, enhanced Wichmann–Hill draws, and adaptive
  JCGM 101 stopping. Polynomial terms and degree are bounded at 128 and 32.
  The sample budget is bounded at 8,000,000. Joint normal sampling requires
  an explicit multivariate normal law, matching normal marginal variances,
  and an exact positive-semidefinite covariance matrix. Exact LDL factors
  retain singular covariance. Rational models require a rectangular-support
  denominator certificate excluding zero. Both model kinds retain the
  centered exact expansion before binary64 sampling.

The fixtures project sealed Ruby F40 and Annex reference records onto this
request shape. Engine identity reports the Rust implementation. These checks
cover selected GUM methods and published input profiles. The full F40 corpus,
complete Annex H acceptance, and named quantity-kind classification remain
outside this scope.

### Conformance decisions

The GUM method `conformity` accepts exact estimates, standard uncertainty,
lower and upper tolerance limits, and an explicit rule. The existing Ruby
rules are `simple`, `guarded_acceptance`, `guarded_rejection`,
`iso14253_style`, and `three_zone`. An open limit is `null`. The result
reports both signed residuals, a signed guard band, expanded uncertainty,
and the decision. Probabilities use the declared decimal-place precision.
This profile requires measurement-only normal knowledge and specific risk.

The source rule includes the inward acceptance edge. The `three_zone` rule
keeps the outward guard edge conditional and rejects points beyond it.
`boundary` records exact equality to an applicable edge independently of
the decision. Frozen F40 case `producer-20` covers outward equality.
