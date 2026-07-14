//! Deterministic fixed-point helpers for host-side simulation math.
//!
//! [`Q16`] provides Q16.16 arithmetic suitable for reproducible ECS-side calculations.

mod q16;

pub use q16::{Q16Error, Q16};
