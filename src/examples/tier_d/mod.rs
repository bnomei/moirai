//! Tier D: integrate persistent host state, diagnostics, scratch data, and replay.
//!
//! Begin with [`d01_system_locals`]. The final replay lesson requires the `testkit`
//! feature and is included in docs.rs builds.

pub mod d01_system_locals;
pub mod d02_diagnostics;
pub mod d03_dense_entity_scratch;
pub mod d04_fixed_point_values;
#[cfg(feature = "testkit")]
pub mod d05_deterministic_replay;
