//! # D04 — Use deterministic fixed-point values
//!
//! **Goal:** perform checked Q16.16 arithmetic without exposing representation details.
//!
//! ```
//! use moirai::Q16;
//!
//! let speed = Q16::try_from_f32(1.5).unwrap();
//! let seconds = Q16::from_i32(2).unwrap();
//! let distance = speed.checked_mul(seconds).unwrap();
//!
//! assert_eq!(distance, Q16::from_i32(3).unwrap());
//! assert_eq!(distance.to_f32(), 3.0);
//! assert!(Q16::ONE.checked_div(Q16::ZERO).is_err());
//! ```
//!
//! `Q16` stores a private Q16.16 representation and makes overflow, underflow, and
//! division by zero explicit. Saturating variants are available when clamping is intended.

#![cfg_attr(
    feature = "testkit",
    doc = "**Next:** continue with [`d05_deterministic_replay`](super::d05_deterministic_replay), or return to [`crate::examples`] for the tier index."
)]
#![cfg_attr(
    not(feature = "testkit"),
    doc = "**Next:** enable `testkit` to unlock the deterministic replay lesson, or return to [`crate::examples`] for the tier index."
)]
