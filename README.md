# Plurimath for Rust

This workspace contains two native libraries:

| Crate | Library | Purpose |
| --- | --- | --- |
| `plurimath-facade` | `plurimath_facade` | Parse formulas into the Plurimath common model and render them through source-backed writers. |
| `plurimath-numeric` | `plurimath_numeric` | Evaluate numeric and GUM JSON requests; enable `quantity` for unit-aware profiles. |

The format facade is being migrated against the Ruby source contract. Its common model uses `{class, fields}` and preserves mixed values; storing a model does not validate every class invariant. Explicit capability errors identify remaining work. Universal format or generated-class parity is not claimed.

Both libraries expose Rust JSON entrypoints and separate version-1 C ABIs with owned result/free functions. The format API uses `op: "parse"` and `op: "write"`; the evaluator uses its versioned numeric, quantity and GUM request schemas. Their models and request protocols are distinct.

```rust
let result = plurimath_facade::request_json(
    br#"{"op":"parse","format":"asciimath","input":"x+y"}"#,
);
assert_eq!(result["ok"], true);
```

See the [format facade](plurimath-facade/README.md), [numeric evaluator](plurimath-numeric/README.md), and their `include/` directories for request examples, limits and ownership rules. Each request carries its options explicitly.

Build and test from this directory with Rust 1.95:

```sh
cargo test --locked --workspace --all-features
cargo test --locked -p plurimath-numeric --no-default-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

All external source dependencies are pinned in the manifests and root `Cargo.lock`. The internal `plurimath-json` crate supplies bounded JSON decoding for both libraries. Build the complete workspace source rather than copying a single member crate in isolation.

The GitHub Actions workflow tests Linux and Windows. It does not publish packages or releases.
