//! GPU backend support for plurimath-rs (issue #20), first slice.
//!
//! This slice is adapter-free: it chooses an honest arithmetic
//! representation for a requested accuracy, reports a rigorous error
//! bound, refuses when the accuracy cannot be honoured, and guards
//! against denormal/overflow values that GPUs flush or saturate.
//! Everything is enabled by the `gpu` feature and tested on the CPU.

#[cfg(feature = "gpu")]
pub mod eft;
#[cfg(feature = "gpu")]
pub mod guard;
#[cfg(feature = "gpu")]
pub mod repr;
