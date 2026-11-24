//! Backend selection: use the device when an adapter exists and the
//! guards accept the requested accuracy, otherwise fall back to the CPU
//! gum-par engine and say why.

use crate::repr::{select, Capabilities, Refusal, Selection};
use serde_json::{json, Value};

/// Where a computation runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    /// Device kernels in the selected representation.
    Device(Selection),
    /// CPU gum-par engine (counter-based Philox, thread-count independent).
    CpuGumPar,
}

/// Why the CPU fallback was taken.
#[derive(Clone, Debug, PartialEq)]
pub enum FallbackReason {
    /// No adapter was found.
    NoAdapter,
    /// The adapter cannot honour the requested accuracy (or the request
    /// was unusable); never degraded silently.
    Refused(Refusal),
}

/// The chosen backend and, for the fallback, the reported reason.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub backend: Backend,
    pub reason: Option<FallbackReason>,
}

/// Plan a computation of `n` accumulated operations at relative
/// tolerance `rel_tol`. `adapter` is `None` when no device was found.
pub fn plan(rel_tol: f64, n: u64, adapter: Option<Capabilities>) -> Plan {
    let Some(caps) = adapter else {
        return Plan {
            backend: Backend::CpuGumPar,
            reason: Some(FallbackReason::NoAdapter),
        };
    };
    match select(rel_tol, n, caps) {
        Ok(sel) => Plan {
            backend: Backend::Device(sel),
            reason: None,
        },
        Err(refusal) => Plan {
            backend: Backend::CpuGumPar,
            reason: Some(FallbackReason::Refused(refusal)),
        },
    }
}

/// Run a Monte Carlo `evaluate.gum/1` request on the CPU gum-par engine.
/// The request must not carry WH `seeds`; engine, seed and threads are set
/// here. Returns the parsed response (check `ok` as usual).
pub fn run_gum_par(request: &Value, seed: u64, threads: u64) -> Value {
    let mut request = request.clone();
    if let Some(parameters) = request.get_mut("parameters").and_then(Value::as_object_mut) {
        parameters.remove("seeds");
        parameters.insert("engine".into(), json!("gum-par"));
        parameters.insert("seed".into(), json!(seed));
        parameters.insert("threads".into(), json!(threads));
    }
    serde_json::from_str(&plurimath_numeric::evaluate_json(&request.to_string())).unwrap_or_else(
        |e| json!({"ok": false, "error": {"code": "Internal", "message": e.to_string()}}),
    )
}
