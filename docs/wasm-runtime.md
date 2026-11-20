# Facade WASM runtime gate

The `WASM` workflow executes the built facade in Node 24, using the official
web glue from the exact hash-verified wasm-bindgen version in `Cargo.lock`.
Every selected request is compared byte for byte with the native `request`
example built from the same source. The focused probe also checks the frozen
Ruby common models and AsciiMath/LaTeX/UnicodeMath writer bytes in `plurimath-facade/fixtures/native-slice-ruby.json`,
AsciiMath/LaTeX/UnicodeMath parsing, UTF-8, malformed requests and continued use
after releasing inputs/results. It is a runtime gate, not full corpus or
cross-language generated LML parity.

On success, `plurimath-facade-wasm-runtime` contains `facade_bg.wasm`, the
matching official `facade.js`/types, `Cargo.lock`, license/notice, a file-digest
and source/tool provenance record, and `runtime-check.json`. The workflow
checks out the PR head explicitly, so `provenance.json` identifies the source
revision consumers must pin. This CI artifact does not publish an npm package.
To repeat the check on an existing build:

```sh
node scripts/check_facade_wasm.mjs --wasm /path/to/facade_bg.wasm \
  --glue /path/to/facade.js --native target/release/examples/request
```

Use `--wasm-only` instead of `--native` only for a local runtime smoke probe;
it reports `nativeDifferential: false` and provides no same-head native parity
evidence. CI always requires the native comparison.
