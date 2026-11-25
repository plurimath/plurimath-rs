//! Representation choice and error-bound reporting.

/// Arithmetic representation a device kernel can use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Repr {
    /// Plain f32 (unit roundoff 2^-24).
    F32,
    /// Double-float, a pair of f32 (conservative unit roundoff 2^-44).
    DoubleFloat,
    /// Native f64 (`shader-f64`; unit roundoff 2^-53).
    NativeF64,
}

impl Repr {
    pub const ALL: [Repr; 3] = [Repr::F32, Repr::DoubleFloat, Repr::NativeF64];

    /// Unit roundoff `u` (relative error per operation).
    pub fn unit_roundoff(self) -> f64 {
        match self {
            Repr::F32 => (-24f64).exp2(),
            Repr::DoubleFloat => (-44f64).exp2(),
            Repr::NativeF64 => (-53f64).exp2(),
        }
    }
}

/// What the adapter offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// Adapter exposes native f64 in shaders.
    pub shader_f64: bool,
    /// Hardware FMA is exact (required for double-float products).
    pub exact_fma: bool,
}

impl Capabilities {
    pub fn supports(self, r: Repr) -> bool {
        match r {
            Repr::F32 => true,
            Repr::DoubleFloat => self.exact_fma,
            Repr::NativeF64 => self.shader_f64,
        }
    }
}

/// Why a request cannot be honoured.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// Tolerance or operation count is not a usable number.
    InvalidRequest,
    /// The operation count is so large that `n*u >= 1`: no bound exists.
    BoundVacuous { repr: Repr },
    /// No representation the adapter supports meets the tolerance.
    AccuracyUnattainable { requested: f64, best_bound: f64 },
}

/// A chosen representation with its rigorous relative error bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub repr: Repr,
    /// Forward relative error bound for `n` accumulated operations.
    pub error_bound: f64,
}

/// Higham's gamma_n = n*u / (1 - n*u): relative error bound for a
/// recursive sum or dot product of `n` terms. `None` when `n*u >= 1`.
pub fn gamma(n: u64, repr: Repr) -> Option<f64> {
    let nu = n as f64 * repr.unit_roundoff();
    if nu < 1.0 {
        Some(nu / (1.0 - nu))
    } else {
        None
    }
}

/// Choose the cheapest supported representation whose bound for `n`
/// operations is within `rel_tol`; otherwise refuse (never degrade
/// silently).
pub fn select(rel_tol: f64, n: u64, caps: Capabilities) -> Result<Selection, Refusal> {
    if !rel_tol.is_finite() || rel_tol <= 0.0 || n == 0 {
        return Err(Refusal::InvalidRequest);
    }
    let mut best = f64::INFINITY;
    let mut vacuous = None;
    for r in Repr::ALL {
        if !caps.supports(r) {
            continue;
        }
        match gamma(n, r) {
            Some(b) if b <= rel_tol => {
                return Ok(Selection {
                    repr: r,
                    error_bound: b,
                })
            }
            Some(b) => best = best.min(b),
            None => vacuous = Some(r),
        }
    }
    if best.is_infinite() {
        return Err(Refusal::BoundVacuous {
            repr: vacuous.unwrap_or(Repr::F32),
        });
    }
    Err(Refusal::AccuracyUnattainable {
        requested: rel_tol,
        best_bound: best,
    })
}

/// Typed cross-backend tolerance check (not bit-identity): the device
/// result must agree with the CPU reference within the selection's bound
/// scaled by the condition magnitude `sum |terms|`.
pub fn within_tolerance(cpu: f64, device: f64, abs_sum: f64, sel: Selection) -> bool {
    cpu.is_finite() && device.is_finite() && (cpu - device).abs() <= sel.error_bound * abs_sum.abs()
}
